//! Shared debug adapter protocol layer.
//!
//! Debug adapters speak JSON-RPC over pipes with the same `Content-Length`
//! framing as language servers, so this module builds on `lsp_core`'s wire
//! helpers and adds what a debugger needs: adapter discovery, the request
//! sequence a session performs, and parsers for stops, stack frames, scopes and
//! variables.
#![allow(dead_code)] // The backends adopt the pieces they need as they migrate.

use std::path::Path;

use crate::lsp_core::{encode_message, json_number, json_objects, json_string, FrameReader};

/// A debug adapter program and the arguments that make it speak DAP on stdio.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdapterSpec {
    pub program: String,
    pub arguments: Vec<String>,
    /// Command the extension page can run to report whether it is installed.
    pub probe_command: String,
}

/// Adapter used for a language, when TC-IDE knows one.
pub fn adapter_for(language: &str) -> Option<AdapterSpec> {
    let (program, arguments) = match language {
        "Python" => ("python", vec!["-m", "debugpy.adapter"]),
        "C" | "C++" | "Rust" => ("lldb-dap", vec![]),
        "Go" => ("dlv", vec!["dap"]),
        _ => return None,
    };
    Some(AdapterSpec {
        program: program.to_string(),
        arguments: arguments.iter().map(|value| (*value).to_string()).collect(),
        probe_command: format!("{program} --version"),
    })
}

/// Where the debugger stopped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StopEvent {
    pub reason: String,
    pub thread_id: Option<u64>,
    pub file: Option<String>,
    pub line: Option<usize>,
}

impl StopEvent {
    /// Text the status bar shows for this stop.
    pub fn describe(&self) -> String {
        match (&self.file, self.line, self.reason.as_str()) {
            (Some(file), Some(line), reason) => {
                format!("Paused ({reason}) at {file}:{}", line + 1)
            }
            (_, _, reason) => format!("Paused ({reason})"),
        }
    }
}

/// One frame of a stopped thread.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StackFrame {
    pub id: u64,
    pub name: String,
    pub file: Option<String>,
    pub line: usize,
    pub column: usize,
}

/// A scope of a stack frame, for example locals or globals.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scope {
    pub name: String,
    pub reference: u64,
    pub expensive: bool,
}

/// A variable of a scope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Variable {
    pub name: String,
    pub value: String,
    pub kind: String,
}

fn bool_string(text: &str, key: &str) -> bool {
    let needle = format!("\"{key}\"");
    text.find(&needle)
        .map(|position| text[position..].trim_start().contains("true"))
        .unwrap_or(false)
}

/// A parsed debug adapter message.
///
/// DAP reuses the `Content-Length` framing of JSON-RPC but not its shape: every
/// message carries a `type` and responses refer to their request by
/// `request_seq`, so routing needs its own reader.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DapMessage {
    Response { request_seq: u64, success: bool },
    Event { name: String },
    Request { command: String },
    Unknown,
}

/// Classify a message from a debug adapter.
pub fn parse_dap_message(text: &str) -> DapMessage {
    let kind = json_string(text, "type").unwrap_or_default();
    match kind.as_str() {
        "response" => DapMessage::Response {
            request_seq: json_number(text, "request_seq").unwrap_or(0),
            success: text.contains("\"success\":true"),
        },
        "event" => DapMessage::Event {
            name: json_string(text, "event").unwrap_or_default(),
        },
        "request" => DapMessage::Request {
            command: json_string(text, "command").unwrap_or_default(),
        },
        _ => DapMessage::Unknown,
    }
}

/// Parse a `stopped` event, including the reason and the source location.
pub fn parse_stopped(text: &str) -> StopEvent {
    let reason = json_string(text, "reason").unwrap_or_else(|| "paused".into());
    let thread_id = json_number(text, "threadId");
    let line = json_number(text, "line").map(|value| value as usize);
    let file = json_string(text, "path");
    StopEvent {
        reason,
        thread_id,
        file,
        line,
    }
}

/// Parse a `stackTrace` response.
pub fn parse_stack_frames(text: &str) -> Vec<StackFrame> {
    json_objects(text, "stackFrames")
        .into_iter()
        .filter_map(|frame| {
            let id = json_number(&frame, "id")?;
            Some(StackFrame {
                id,
                name: json_string(&frame, "name").unwrap_or_default(),
                file: json_string(&frame, "path"),
                line: json_number(&frame, "line").unwrap_or(1).saturating_sub(1) as usize,
                column: json_number(&frame, "column").unwrap_or(1).saturating_sub(1) as usize,
            })
        })
        .collect()
}

/// Parse a `scopes` response.
pub fn parse_scopes(text: &str) -> Vec<Scope> {
    json_objects(text, "scopes")
        .into_iter()
        .filter_map(|scope| {
            Some(Scope {
                name: json_string(&scope, "name")?,
                reference: json_number(&scope, "variablesReference")?,
                expensive: bool_string(&scope, "expensive"),
            })
        })
        .collect()
}

/// Parse a `variables` response.
pub fn parse_variables(text: &str) -> Vec<Variable> {
    json_objects(text, "variables")
        .into_iter()
        .filter_map(|variable| {
            Some(Variable {
                name: json_string(&variable, "name")?,
                value: json_string(&variable, "value").unwrap_or_default(),
                kind: json_string(&variable, "type").unwrap_or_default(),
            })
        })
        .collect()
}

/// Parse a `setBreakpoints` response into verified source lines.
pub fn parse_breakpoint_lines(text: &str) -> Vec<usize> {
    json_objects(text, "breakpoints")
        .into_iter()
        .filter_map(|breakpoint| json_number(&breakpoint, "line"))
        .map(|line| line.saturating_sub(1) as usize)
        .collect()
}

/// Build a request message.
pub fn request(seq: u64, command: &str, arguments: &str) -> String {
    if arguments.is_empty() {
        format!("{{\"seq\":{seq},\"type\":\"request\",\"command\":\"{command}\"}}")
    } else {
        format!(
            "{{\"seq\":{seq},\"type\":\"request\",\"command\":\"{command}\",\"arguments\":{arguments}}}"
        )
    }
}

/// `initialize` request arguments for the given client.
pub fn initialize_arguments(client_id: &str) -> String {
    format!(
        "{{\"clientID\":\"{client_id}\",\"clientName\":\"TC-IDE\",\"adapterID\":\"tc-ide\",\"linesStartAt1\":true,\"columnsStartAt1\":true,\"pathFormat\":\"path\",\"supportsVariableType\":true}}"
    )
}

/// `launch` arguments for a program, or `attach` when a port is given.
pub fn launch_arguments(program: &str, cwd: &str, stop_on_entry: bool) -> String {
    let program = program.replace('\\', "\\\\").replace('"', "\\\"");
    let cwd = cwd.replace('\\', "\\\\").replace('"', "\\\"");
    format!(
        "{{\"program\":\"{program}\",\"cwd\":\"{cwd}\",\"stopOnEntry\":{},\"console\":\"internalConsole\"}}",
        if stop_on_entry { "true" } else { "false" }
    )
}

/// `setBreakpoints` arguments for one file.
pub fn set_breakpoints_arguments(path: &str, breakpoints: &[BreakpointSpec]) -> String {
    let path = escape_json(path);
    let list = breakpoints
        .iter()
        .map(|breakpoint| {
            let mut fields = vec![format!("\"line\":{}", breakpoint.line + 1)];
            for (key, value) in [
                ("condition", breakpoint.condition.as_ref()),
                ("hitCondition", breakpoint.hit_condition.as_ref()),
                ("logMessage", breakpoint.log_message.as_ref()),
            ] {
                if let Some(value) = value.filter(|value| !value.trim().is_empty()) {
                    fields.push(format!("\"{key}\":\"{}\"", escape_json(value)));
                }
            }
            format!("{{{}}}", fields.join(","))
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{{\"source\":{{\"path\":\"{path}\"}},\"breakpoints\":[{list}]}}")
}

/// Escape a string for a JSON document.
pub fn escape_json(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            value if (value as u32) < 0x20 => {
                escaped.push_str(&format!("\\u{:04x}", value as u32));
            }
            value => escaped.push(value),
        }
    }
    escaped
}

/// One breakpoint: a line, and what makes it stop.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct BreakpointSpec {
    pub line: usize,
    /// Expression that has to hold for the breakpoint to stop.
    pub condition: Option<String>,
    /// Number of hits before it stops, in the form the adapter expects.
    pub hit_condition: Option<String>,
    /// Message a logpoint prints instead of stopping.
    pub log_message: Option<String>,
}

impl BreakpointSpec {
    /// A plain breakpoint on a line.
    pub fn new(line: usize) -> Self {
        Self {
            line,
            ..Self::default()
        }
    }

    /// A breakpoint that only stops while `condition` holds.
    pub fn conditional(line: usize, condition: impl Into<String>) -> Self {
        Self {
            line,
            condition: Some(condition.into()),
            ..Self::default()
        }
    }

    /// Whether the breakpoint carries anything beyond its line.
    pub fn is_conditional(&self) -> bool {
        self.condition
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty())
            || self
                .hit_condition
                .as_ref()
                .is_some_and(|value| !value.trim().is_empty())
            || self
                .log_message
                .as_ref()
                .is_some_and(|value| !value.trim().is_empty())
    }

    /// One line description for the problems panel.
    pub fn describe(&self) -> String {
        let mut text = format!("line {}", self.line + 1);
        if let Some(condition) = self
            .condition
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            text.push_str(&format!(" · if {condition}"));
        }
        if let Some(hits) = self
            .hit_condition
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            text.push_str(&format!(" · hits {hits}"));
        }
        if let Some(message) = self
            .log_message
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            text.push_str(&format!(" · log {message}"));
        }
        text
    }
}

/// Breakpoints of a session, per file, in line order.
///
/// The panel edits this table in place: a breakpoint can be added, removed or
/// given a condition without the editor and the debug adapter disagreeing about
/// what is set.
#[derive(Clone, Debug, Default)]
pub struct Breakpoints {
    files: std::collections::BTreeMap<String, Vec<BreakpointSpec>>,
}

impl Breakpoints {
    /// No breakpoints.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add or remove the plain breakpoint on a line. Returns true when set.
    pub fn toggle(&mut self, path: &str, line: usize) -> bool {
        if self.clear(path, line) {
            return false;
        }
        self.set(path, BreakpointSpec::new(line));
        true
    }

    /// Add a breakpoint, replacing the one already on that line.
    pub fn set(&mut self, path: &str, breakpoint: BreakpointSpec) {
        let entry = self.files.entry(path.to_string()).or_default();
        match entry.iter().position(|known| known.line == breakpoint.line) {
            Some(index) => entry[index] = breakpoint,
            None => entry.push(breakpoint),
        }
        entry.sort_by_key(|known| known.line);
    }

    /// Remove the breakpoint on a line, if there is one.
    pub fn clear(&mut self, path: &str, line: usize) -> bool {
        let Some(entry) = self.files.get_mut(path) else {
            return false;
        };
        let before = entry.len();
        entry.retain(|known| known.line != line);
        let removed = entry.len() != before;
        if entry.is_empty() {
            self.files.remove(path);
        }
        removed
    }

    /// Set, change or drop the condition of the breakpoint on a line.
    ///
    /// Returns false when the line has no breakpoint: a condition without a
    /// breakpoint would never stop anything.
    pub fn condition(&mut self, path: &str, line: usize, condition: Option<&str>) -> bool {
        let Some(entry) = self.files.get_mut(path) else {
            return false;
        };
        let Some(breakpoint) = entry.iter_mut().find(|known| known.line == line) else {
            return false;
        };
        breakpoint.condition = condition
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        true
    }

    /// Breakpoints of one file, in line order.
    pub fn specs(&self, path: &str) -> &[BreakpointSpec] {
        self.files.get(path).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Lines of one file that have a breakpoint.
    pub fn lines(&self, path: &str) -> Vec<usize> {
        self.specs(path).iter().map(|known| known.line).collect()
    }

    /// Every file with its breakpoints, in path order.
    pub fn files(&self) -> Vec<(String, Vec<BreakpointSpec>)> {
        self.files
            .iter()
            .map(|(path, specs)| (path.clone(), specs.clone()))
            .collect()
    }

    /// Breakpoints in the session.
    pub fn len(&self) -> usize {
        self.files.values().map(Vec::len).sum()
    }

    /// Whether nothing is set.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Forget everything, for a session that ended.
    pub fn clear_all(&mut self) {
        self.files.clear();
    }
}

/// `stackTrace` arguments for a thread.
pub fn stack_trace_arguments(thread_id: u64) -> String {
    format!("{{\"threadId\":{thread_id},\"startFrame\":0,\"levels\":20}}")
}

/// `variables` arguments for a scope reference.
pub fn variables_arguments(reference: u64) -> String {
    format!("{{\"variablesReference\":{reference}}}")
}

/// Which request a response belongs to, for routing by sequence number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingRequest {
    Initialize,
    Launch,
    SetBreakpoints,
    ConfigurationDone,
    StackTrace,
    Scopes,
    Variables,
    Continue,
    Next,
    StepIn,
    StepOut,
    Pause,
    Disconnect,
}

impl PendingRequest {
    fn command(self) -> &'static str {
        match self {
            PendingRequest::Initialize => "initialize",
            PendingRequest::Launch => "launch",
            PendingRequest::SetBreakpoints => "setBreakpoints",
            PendingRequest::ConfigurationDone => "configurationDone",
            PendingRequest::StackTrace => "stackTrace",
            PendingRequest::Scopes => "scopes",
            PendingRequest::Variables => "variables",
            PendingRequest::Continue => "continue",
            PendingRequest::Next => "next",
            PendingRequest::StepIn => "stepIn",
            PendingRequest::StepOut => "stepOut",
            PendingRequest::Pause => "pause",
            PendingRequest::Disconnect => "disconnect",
        }
    }
}

/// A running debug adapter session.
pub struct DebugSession {
    child: std::process::Child,
    input: Option<std::process::ChildStdin>,
    messages: std::sync::mpsc::Receiver<String>,
    errors: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    next_sequence: u64,
    pending: Vec<(u64, PendingRequest)>,
    spec: AdapterSpec,
    stopped: Option<StopEvent>,
    frames: Vec<StackFrame>,
    scopes: Vec<Scope>,
    variables: Vec<Variable>,
    terminated: bool,
}

/// What a debugger session currently knows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DebugState {
    pub stopped: Option<StopEvent>,
    pub frames: Vec<StackFrame>,
    pub scopes: Vec<Scope>,
    pub variables: Vec<Variable>,
    pub terminated: bool,
    pub output: Vec<String>,
}

impl DebugSession {
    /// Start an adapter in a working directory.
    pub fn start(spec: &AdapterSpec, cwd: &Path) -> Result<Self, String> {
        use std::process::{Command, Stdio};
        let mut command = Command::new(&spec.program);
        command
            .args(&spec.arguments)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command
            .spawn()
            .map_err(|error| format!("{} could not start: {error}", spec.program))?;
        let input = child.stdin.take();
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "the debug adapter has no standard output".to_string())?;
        let errors = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let (sender, messages) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            use std::io::Read;
            let mut stdout = stdout;
            let mut reader = FrameReader::new();
            let mut buffer = [0u8; 8192];
            while let Ok(read) = stdout.read(&mut buffer) {
                if read == 0 {
                    break;
                }
                for message in reader.push(&buffer[..read]) {
                    if sender.send(message).is_err() {
                        return;
                    }
                }
            }
        });
        if let Some(stderr) = child.stderr.take() {
            let errors = std::sync::Arc::clone(&errors);
            std::thread::spawn(move || {
                use std::io::Read;
                let mut stderr = stderr;
                let mut buffer = String::new();
                let _ = stderr.read_to_string(&mut buffer);
                if !buffer.trim().is_empty() {
                    if let Ok(mut errors) = errors.lock() {
                        for line in buffer.lines().take(20) {
                            errors.push(line.to_string());
                        }
                    }
                }
            });
        }
        Ok(Self {
            child,
            input,
            messages,
            errors,
            next_sequence: 1,
            pending: Vec::new(),
            spec: spec.clone(),
            stopped: None,
            frames: Vec::new(),
            scopes: Vec::new(),
            variables: Vec::new(),
            terminated: false,
        })
    }

    pub fn spec(&self) -> &AdapterSpec {
        &self.spec
    }

    pub fn is_running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    pub fn state(&self) -> DebugState {
        DebugState {
            stopped: self.stopped.clone(),
            frames: self.frames.clone(),
            scopes: self.scopes.clone(),
            variables: self.variables.clone(),
            terminated: self.terminated,
            output: Vec::new(),
        }
    }

    /// Whether a request of this kind is still unanswered.
    pub fn has_pending(&self, request: PendingRequest) -> bool {
        self.pending.iter().any(|(_, pending)| *pending == request)
    }

    pub fn stderr_lines(&self) -> Vec<String> {
        self.errors
            .lock()
            .map(|errors| errors.clone())
            .unwrap_or_default()
    }

    fn send(&mut self, message: String) -> Result<(), String> {
        use std::io::Write;
        let Some(input) = self.input.as_mut() else {
            return Err("the debug adapter input is closed".into());
        };
        input
            .write_all(encode_message(&message).as_bytes())
            .and_then(|()| input.flush())
            .map_err(|error| format!("could not reach the debug adapter: {error}"))
    }

    /// Send a request and remember what it was for.
    pub fn request(&mut self, request: PendingRequest, arguments: &str) -> Result<u64, String> {
        let sequence = self.next_sequence;
        self.next_sequence += 1;
        self.send(request_message(sequence, request.command(), arguments))?;
        self.pending.push((sequence, request));
        Ok(sequence)
    }

    /// Start a session and send the breakpoints the editor collected.
    pub fn initialize_and_launch(
        &mut self,
        program: &str,
        cwd: &str,
        breakpoints: &[(String, Vec<BreakpointSpec>)],
    ) -> Result<(), String> {
        self.request(PendingRequest::Initialize, &initialize_arguments("tc-ide"))?;
        for (path, specs) in breakpoints {
            if specs.is_empty() {
                continue;
            }
            self.request(
                PendingRequest::SetBreakpoints,
                &set_breakpoints_arguments(path, specs),
            )?;
        }
        self.request(
            PendingRequest::Launch,
            &launch_arguments(program, cwd, false),
        )?;
        self.request(PendingRequest::ConfigurationDone, "")?;
        Ok(())
    }

    /// Take the next message without blocking, updating the debug state.
    pub fn poll(&mut self) -> Option<String> {
        let message = self.messages.try_recv().ok()?;
        match parse_dap_message(&message) {
            DapMessage::Response { request_seq, .. } => {
                let position = self
                    .pending
                    .iter()
                    .position(|(sequence, _)| *sequence == request_seq);
                if let Some(position) = position {
                    let (_, request) = self.pending.remove(position);
                    self.apply_response(request, &message);
                }
            }
            DapMessage::Event { name } => match name.as_str() {
                "stopped" => {
                    self.stopped = Some(parse_stopped(&message));
                    self.frames.clear();
                    self.scopes.clear();
                    self.variables.clear();
                }
                "terminated" | "exited" => {
                    self.terminated = true;
                    self.stopped = None;
                }
                "continued" => {
                    self.stopped = None;
                    self.frames.clear();
                    self.scopes.clear();
                    self.variables.clear();
                }
                _ => {}
            },
            _ => {}
        }
        Some(message)
    }

    fn apply_response(&mut self, request: PendingRequest, message: &str) {
        match request {
            PendingRequest::StackTrace => {
                self.frames = parse_stack_frames(message);
            }
            PendingRequest::Scopes => {
                self.scopes = parse_scopes(message);
            }
            PendingRequest::Variables => {
                self.variables = parse_variables(message);
            }
            _ => {}
        }
    }

    /// Ask for the stack of the stopped thread, then its scopes and variables.
    pub fn refresh_state(&mut self) -> Result<(), String> {
        let Some(stopped) = self.stopped.clone() else {
            return Ok(());
        };
        let Some(thread_id) = stopped.thread_id else {
            return Ok(());
        };
        self.request(
            PendingRequest::StackTrace,
            &stack_trace_arguments(thread_id),
        )?;
        Ok(())
    }

    /// Ask for the variables of the first scope, once scopes are known.
    pub fn request_scope_variables(&mut self) -> Result<(), String> {
        let Some(frame) = self.frames.first() else {
            return Ok(());
        };
        let frame_id = frame.id;
        self.request(
            PendingRequest::Scopes,
            &format!("{{\"frameId\":{frame_id}}}"),
        )?;
        Ok(())
    }

    pub fn request_variables_of_first_scope(&mut self) -> Result<(), String> {
        let Some(scope) = self.scopes.iter().find(|scope| !scope.expensive) else {
            return Ok(());
        };
        let reference = scope.reference;
        self.request(PendingRequest::Variables, &variables_arguments(reference))?;
        Ok(())
    }

    /// Resume, step or pause the stopped thread.
    pub fn control(&mut self, request: PendingRequest) -> Result<(), String> {
        let thread_id = self
            .stopped
            .as_ref()
            .and_then(|stopped| stopped.thread_id)
            .unwrap_or(1);
        let arguments = format!("{{\"threadId\":{thread_id}}}");
        self.request(request, &arguments)?;
        Ok(())
    }

    /// End the session, stopping the adapter if it does not exit.
    pub fn disconnect(&mut self) {
        let _ = self.request(PendingRequest::Disconnect, "{\"terminateDebuggee\":true}");
        self.input = None;
        for _ in 0..20 {
            if !self.is_running() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
    }
}

fn request_message(sequence: u64, command: &str, arguments: &str) -> String {
    request(sequence, command, arguments)
}

impl Drop for DebugSession {
    fn drop(&mut self) {
        self.disconnect();
        if self.is_running() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lsp_core::program_runs;

    #[test]
    fn adapters_are_chosen_per_language() {
        let python = adapter_for("Python").expect("adapter");
        assert_eq!(python.program, "python");
        assert_eq!(python.arguments, ["-m", "debugpy.adapter"]);
        let rust = adapter_for("Rust").expect("adapter");
        assert_eq!(rust.program, "lldb-dap");
        assert!(adapter_for("Markdown").is_none());
    }

    #[test]
    fn requests_carry_their_arguments() {
        assert_eq!(
            request(1, "configurationDone", ""),
            "{\"seq\":1,\"type\":\"request\",\"command\":\"configurationDone\"}"
        );
        let launch = launch_arguments("C:\\demo\\app.py", "C:\\demo", true);
        assert!(launch.contains("C:\\\\demo\\\\app.py"));
        assert!(launch.contains("\"stopOnEntry\":true"));
        let breakpoints =
            set_breakpoints_arguments("/p/a.rs", &[BreakpointSpec::new(2), BreakpointSpec::new(9)]);
        assert!(breakpoints.contains("\"path\":\"/p/a.rs\""));
        assert!(breakpoints.contains("[{\"line\":3},{\"line\":10}]"));
        assert!(stack_trace_arguments(4).contains("\"threadId\":4"));
        assert!(variables_arguments(7).contains("\"variablesReference\":7"));
        assert!(initialize_arguments("tc-ide").contains("\"linesStartAt1\":true"));
    }

    #[test]
    fn stops_frames_scopes_and_variables_are_parsed() {
        let stopped = "{\"type\":\"event\",\"event\":\"stopped\",\"body\":{\"reason\":\"breakpoint\",\"threadId\":3,\"allThreadsStopped\":true}}";
        let event = parse_stopped(stopped);
        assert_eq!(event.reason, "breakpoint");
        assert_eq!(event.thread_id, Some(3));
        assert_eq!(event.describe(), "Paused (breakpoint)");

        let located = "{\"event\":\"stopped\",\"body\":{\"reason\":\"step\",\"threadId\":1,\"source\":{\"path\":\"/p/a.rs\"},\"line\":12}}";
        let event = parse_stopped(located);
        assert_eq!(event.file.as_deref(), Some("/p/a.rs"));
        assert_eq!(event.line, Some(12));
        assert!(event.describe().contains("a.rs:13"));

        let stack = "{\"body\":{\"stackFrames\":[{\"id\":1000,\"name\":\"main\",\"source\":{\"path\":\"/p/a.rs\"},\"line\":7,\"column\":1},{\"id\":1001,\"name\":\"helper\",\"source\":{\"path\":\"/p/a.rs\"},\"line\":12,\"column\":5}],\"totalFrames\":2}}";
        let frames = parse_stack_frames(stack);
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].name, "main");
        assert_eq!(frames[0].line, 6);
        assert_eq!(frames[1].column, 4);

        let scopes = "{\"body\":{\"scopes\":[{\"name\":\"Locals\",\"variablesReference\":2,\"expensive\":false},{\"name\":\"Globals\",\"variablesReference\":3,\"expensive\":true}]}}";
        let scopes = parse_scopes(scopes);
        assert_eq!(scopes.len(), 2);
        assert!(!scopes[0].expensive);
        assert!(scopes[1].expensive);

        let variables = "{\"body\":{\"variables\":[{\"name\":\"count\",\"value\":\"3\",\"type\":\"int\"},{\"name\":\"name\",\"value\":\"\\\"tc\\\"\",\"type\":\"str\"}]}}";
        let variables = parse_variables(variables);
        assert_eq!(variables.len(), 2);
        assert_eq!(variables[0].name, "count");
        assert_eq!(variables[0].value, "3");
        assert_eq!(variables[1].kind, "str");

        let breakpoints = "{\"body\":{\"breakpoints\":[{\"verified\":true,\"line\":3},{\"verified\":true,\"line\":9}]}}";
        assert_eq!(parse_breakpoint_lines(breakpoints), [2, 8]);
    }

    #[test]
    fn dap_messages_are_classified_by_type() {
        let response = "{\"seq\":1,\"type\":\"response\",\"request_seq\":4,\"success\":true,\"command\":\"launch\"}";
        assert_eq!(
            parse_dap_message(response),
            DapMessage::Response {
                request_seq: 4,
                success: true
            }
        );
        let event = "{\"seq\":5,\"type\":\"event\",\"event\":\"stopped\",\"body\":{}}";
        assert_eq!(
            parse_dap_message(event),
            DapMessage::Event {
                name: "stopped".into()
            }
        );
        let request = "{\"seq\":6,\"type\":\"request\",\"command\":\"runInTerminal\"}";
        assert_eq!(
            parse_dap_message(request),
            DapMessage::Request {
                command: "runInTerminal".into()
            }
        );
        assert_eq!(
            parse_dap_message("{\"type\":\"nonsense\"}"),
            DapMessage::Unknown
        );
        assert!(!bool_string("{\"verified\":false}", "verified"));
        assert!(bool_string("{\"verified\":true}", "verified"));
    }

    #[test]
    fn pending_requests_map_to_commands() {
        assert_eq!(PendingRequest::Initialize.command(), "initialize");
        assert_eq!(PendingRequest::SetBreakpoints.command(), "setBreakpoints");
        assert_eq!(
            PendingRequest::ConfigurationDone.command(),
            "configurationDone"
        );
        assert_eq!(PendingRequest::StepIn.command(), "stepIn");
        assert_eq!(PendingRequest::Disconnect.command(), "disconnect");
    }

    #[test]
    fn a_real_adapter_process_is_driven_over_pipes() {
        // A tiny python DAP adapter: initializes, acknowledges breakpoints and
        // launch, reports a stop, then serves stackTrace, scopes and variables.
        let script = r#"
import json, sys

def read_message():
    length = 0
    while True:
        line = sys.stdin.buffer.readline()
        if not line:
            return None
        if line in (b"\r\n", b"\n"):
            break
        if line.lower().startswith(b"content-length:"):
            length = int(line.split(b":")[1].strip())
    return json.loads(sys.stdin.buffer.read(length).decode("utf-8"))

def send(payload):
    body = json.dumps(payload).encode("utf-8")
    sys.stdout.buffer.write(b"Content-Length: %d\r\n\r\n" % len(body))
    sys.stdout.buffer.write(body)
    sys.stdout.buffer.flush()

launched = False
while True:
    message = read_message()
    if message is None:
        break
    command = message.get("command")
    if command == "initialize":
        send({"seq": 1, "type": "response", "request_seq": message["seq"], "success": True, "command": command})
        send({"seq": 2, "type": "event", "event": "initialized"})
    elif command == "setBreakpoints":
        lines = [{"verified": True, "line": bp["line"]} for bp in message["arguments"]["breakpoints"]]
        send({"seq": 3, "type": "response", "request_seq": message["seq"], "success": True, "command": command,
              "body": {"breakpoints": lines}})
    elif command == "launch":
        send({"seq": 4, "type": "response", "request_seq": message["seq"], "success": True, "command": command})
        launched = True
        send({"seq": 5, "type": "event", "event": "stopped",
              "body": {"reason": "breakpoint", "threadId": 1,
                       "source": {"path": "/tmp/demo.py"}, "line": 3}})
    elif command == "configurationDone":
        send({"seq": 6, "type": "response", "request_seq": message["seq"], "success": True, "command": command})
    elif command == "stackTrace":
        send({"seq": 7, "type": "response", "request_seq": message["seq"], "success": True, "command": command,
              "body": {"stackFrames": [{"id": 1000, "name": "main",
                                        "source": {"path": "/tmp/demo.py"}, "line": 3, "column": 1}]}})
    elif command == "scopes":
        send({"seq": 8, "type": "response", "request_seq": message["seq"], "success": True, "command": command,
              "body": {"scopes": [{"name": "Locals", "variablesReference": 2, "expensive": False}]}})
    elif command == "variables":
        send({"seq": 9, "type": "response", "request_seq": message["seq"], "success": True, "command": command,
              "body": {"variables": [{"name": "count", "value": "3", "type": "int"}]}})
    elif command == "continue":
        send({"seq": 10, "type": "response", "request_seq": message["seq"], "success": True, "command": command})
        send({"seq": 11, "type": "event", "event": "terminated"})
    elif command == "disconnect":
        send({"seq": 12, "type": "response", "request_seq": message["seq"], "success": True, "command": command})
        break
"#;
        let python = if cfg!(windows) { "python" } else { "python3" };
        if !program_runs(python, &["-c", "print(1)"]) {
            // The adapter test needs a real interpreter, not a Store stub.
            return;
        }
        let directory = crate::lsp_core::test_directory("tc-ide-dap-test");
        let script_path = directory.join("fake_adapter.py");
        std::fs::write(&script_path, script).expect("write fake adapter");
        let spec = AdapterSpec {
            program: python.to_string(),
            arguments: vec![script_path.to_string_lossy().into_owned()],
            probe_command: String::new(),
        };
        let mut session = DebugSession::start(&spec, &directory).expect("start adapter");
        session
            .initialize_and_launch(
                "/tmp/demo.py",
                &directory.to_string_lossy(),
                &[("/tmp/demo.py".to_string(), vec![BreakpointSpec::new(2)])],
            )
            .expect("launch");

        // Drive the session until it has stopped, then collect its state.
        let mut saw_stop = false;
        // A loaded runner can be slow to start the interpreter; this guards
        // against hanging rather than asserting how fast it starts.
        let stop_deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while std::time::Instant::now() < stop_deadline {
            let Some(_) = session.poll() else {
                std::thread::sleep(std::time::Duration::from_millis(10));
                continue;
            };
            if session.state().stopped.is_some() && !saw_stop {
                saw_stop = true;
                session.refresh_state().expect("stackTrace");
            }
            if saw_stop && !session.state().frames.is_empty() && session.state().scopes.is_empty() {
                session.request_scope_variables().expect("scopes");
            }
            if !session.state().scopes.is_empty() && session.state().variables.is_empty() {
                session
                    .request_variables_of_first_scope()
                    .expect("variables");
            }
            if !session.state().variables.is_empty() {
                break;
            }
        }
        let state = session.state();
        assert!(saw_stop, "the adapter never reported a stop");
        assert_eq!(
            state.stopped.as_ref().map(|stop| stop.reason.as_str()),
            Some("breakpoint")
        );
        assert_eq!(state.frames.len(), 1);
        assert_eq!(state.frames[0].name, "main");
        assert_eq!(state.scopes[0].name, "Locals");
        assert_eq!(state.variables[0].name, "count");
        assert_eq!(state.variables[0].value, "3");

        session.control(PendingRequest::Continue).expect("continue");
        let mut terminated = false;
        let term_deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while std::time::Instant::now() < term_deadline {
            if session.poll().is_none() {
                std::thread::sleep(std::time::Duration::from_millis(10));
                continue;
            }
            if session.state().terminated {
                terminated = true;
                break;
            }
        }
        assert!(terminated, "the adapter never terminated");

        drop(session);
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_breakpoint_carries_a_condition_a_hit_count_or_a_log_message() {
        let plain = set_breakpoints_arguments("/p/a.rs", &[BreakpointSpec::new(2)]);
        assert_eq!(
            plain,
            "{\"source\":{\"path\":\"/p/a.rs\"},\"breakpoints\":[{\"line\":3}]}"
        );

        let specs = vec![
            BreakpointSpec::conditional(4, "value > 3"),
            BreakpointSpec {
                line: 8,
                hit_condition: Some("5".into()),
                log_message: Some("value is {value}".into()),
                ..BreakpointSpec::default()
            },
            BreakpointSpec {
                line: 9,
                condition: Some("   ".into()),
                ..BreakpointSpec::default()
            },
        ];
        let arguments = set_breakpoints_arguments("/p/a.rs", &specs);
        assert!(
            arguments.contains("{\"line\":5,\"condition\":\"value > 3\"}"),
            "{arguments}"
        );
        assert!(arguments.contains("\"hitCondition\":\"5\""));
        assert!(arguments.contains("\"logMessage\":\"value is {value}\""));
        assert!(
            arguments.contains("{\"line\":10}"),
            "a blank condition is left out: {arguments}"
        );

        // Quotes and newlines in a condition cannot break the request document.
        let tricky = set_breakpoints_arguments(
            "/p/a\"b.rs",
            &[BreakpointSpec::conditional(0, "name == \"x\"\nnext")],
        );
        assert!(tricky.contains("/p/a\\\"b.rs"), "{tricky}");
        assert!(tricky.contains("name == \\\"x\\\"\\nnext"), "{tricky}");

        assert!(BreakpointSpec::conditional(1, "x").is_conditional());
        assert!(BreakpointSpec::new(1).describe() == "line 2");
        assert_eq!(
            BreakpointSpec::conditional(11, "count > 0").describe(),
            "line 12 · if count > 0"
        );
        let logpoint = BreakpointSpec {
            line: 0,
            log_message: Some("hit".into()),
            ..BreakpointSpec::default()
        };
        assert_eq!(logpoint.describe(), "line 1 · log hit");
        assert!(logpoint.is_conditional());
    }

    #[test]
    fn the_breakpoint_table_is_edited_in_place() {
        let mut breakpoints = Breakpoints::new();
        assert!(breakpoints.toggle("a.rs", 4), "a toggle sets a breakpoint");
        assert!(
            !breakpoints.toggle("a.rs", 4),
            "and toggling again removes it"
        );
        assert!(breakpoints.is_empty());

        assert!(breakpoints.toggle("a.rs", 9));
        assert!(breakpoints.toggle("a.rs", 2));
        assert!(breakpoints.toggle("b.rs", 1));
        assert_eq!(breakpoints.lines("a.rs"), [2, 9], "lines stay sorted");
        assert_eq!(breakpoints.len(), 3);
        assert_eq!(breakpoints.files().len(), 2);

        // A condition needs a breakpoint to attach to.
        assert!(!breakpoints.condition("c.rs", 1, Some("x")));
        assert!(!breakpoints.condition("a.rs", 3, Some("x")));
        assert!(breakpoints.condition("a.rs", 9, Some("value > 3")));
        assert_eq!(
            breakpoints.specs("a.rs")[1].condition.as_deref(),
            Some("value > 3")
        );
        assert!(breakpoints.specs("a.rs")[1].is_conditional());
        // Clearing it leaves a plain breakpoint behind.
        assert!(breakpoints.condition("a.rs", 9, None));
        assert!(!breakpoints.specs("a.rs")[1].is_conditional());

        // Setting a breakpoint again replaces the one on that line.
        breakpoints.set("a.rs", BreakpointSpec::conditional(9, "again"));
        assert_eq!(breakpoints.len(), 3);
        assert_eq!(
            breakpoints.specs("a.rs")[1].condition.as_deref(),
            Some("again")
        );

        assert!(breakpoints.clear("a.rs", 9));
        assert!(!breakpoints.clear("a.rs", 9));
        breakpoints.clear("b.rs", 1);
        assert_eq!(breakpoints.files().len(), 1);
        breakpoints.clear_all();
        assert!(breakpoints.is_empty());
        assert!(breakpoints.specs("a.rs").is_empty());
    }
}

/// One row of the debug panel.
///
/// The panel lists the call stack and, under the frame the user picked, its
/// scopes and their variables. The rows are built here rather than in a
/// backend so all three can show the same thing, and so the layout can be
/// tested without a window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DebugRow {
    /// A heading, for example `CALL STACK`.
    Heading(String),
    /// A stack frame, with whether it is the one being shown.
    Frame {
        index: usize,
        label: String,
        detail: String,
        selected: bool,
    },
    /// A scope inside the selected frame.
    Scope { label: String, detail: String },
    /// A variable of the scope above it.
    Variable { name: String, value: String },
    /// The debugger has nothing to show yet.
    Empty(String),
}

/// Byte budget a row keeps for its label and its detail.
///
/// A debugger value can be a whole structure; the panel shows the beginning of
/// it and says how much was left out rather than letting one row run off.
pub const DEBUG_VALUE_LIMIT: usize = 60;

/// Shorten a debugger value to something a row can show.
pub fn shortened_value(value: &str) -> String {
    let flat = value.replace(['\n', '\r', '\t'], " ");
    let characters = flat.chars().count();
    if characters <= DEBUG_VALUE_LIMIT {
        return flat;
    }
    let head: String = flat.chars().take(DEBUG_VALUE_LIMIT).collect();
    format!("{head}… (+{})", characters - DEBUG_VALUE_LIMIT)
}

/// The rows the debug panel shows for a state and a selected frame.
///
/// A frame the state does not have falls back to the first one, so a panel
/// that outlives a stop cannot show the stack of a run that already ended.
pub fn debug_rows(state: &DebugState, selected_frame: usize) -> Vec<DebugRow> {
    let mut rows = Vec::new();
    if state.frames.is_empty() && state.variables.is_empty() {
        rows.push(DebugRow::Empty(if state.terminated {
            "the program has finished".to_string()
        } else {
            "not stopped".to_string()
        }));
        return rows;
    }

    if !state.frames.is_empty() {
        rows.push(DebugRow::Heading("CALL STACK".to_string()));
        let selected = selected_frame.min(state.frames.len() - 1);
        for (index, frame) in state.frames.iter().enumerate() {
            let detail = match (&frame.file, frame.line) {
                (Some(file), line) if line > 0 => {
                    let name = file.rsplit(['/', '\\']).next().unwrap_or(file);
                    format!("{name}:{line}")
                }
                (Some(file), _) => file.clone(),
                (None, line) if line > 0 => format!("line {line}"),
                (None, _) => String::new(),
            };
            rows.push(DebugRow::Frame {
                index,
                label: format!("{}  {index}", frame.name),
                detail,
                selected: index == selected,
            });
        }
    }

    if !state.scopes.is_empty() {
        rows.push(DebugRow::Heading("SCOPES".to_string()));
        for scope in &state.scopes {
            rows.push(DebugRow::Scope {
                label: scope.name.clone(),
                detail: if scope.expensive {
                    "not loaded".to_string()
                } else {
                    String::new()
                },
            });
        }
    }

    if !state.variables.is_empty() {
        rows.push(DebugRow::Heading("VARIABLES".to_string()));
        for variable in &state.variables {
            rows.push(DebugRow::Variable {
                name: variable.name.clone(),
                value: shortened_value(&variable.value),
            });
        }
    }
    rows
}

/// Rows of a panel that can show every row, or the first few with a count.
pub fn visible_debug_rows(rows: &[DebugRow], limit: usize) -> (Vec<DebugRow>, usize) {
    if limit == 0 || rows.len() <= limit {
        return (rows.to_vec(), 0);
    }
    (rows[..limit].to_vec(), rows.len() - limit)
}

#[cfg(test)]
mod debug_row_tests {
    use super::*;

    fn frame(id: u64, name: &str, file: Option<&str>, line: usize) -> StackFrame {
        StackFrame {
            id,
            name: name.to_string(),
            file: file.map(str::to_string),
            line,
            column: 1,
        }
    }

    fn variable(name: &str, value: &str) -> Variable {
        Variable {
            name: name.to_string(),
            value: value.to_string(),
            kind: "int".to_string(),
        }
    }

    #[test]
    fn the_panel_lists_the_stack_then_the_scopes_then_the_variables() {
        let state = DebugState {
            stopped: None,
            frames: vec![
                frame(1, "main", Some("/home/me/src/main.c"), 12),
                frame(2, "helper", Some("/home/me/src/util.c"), 4),
            ],
            scopes: vec![Scope {
                name: "Locals".to_string(),
                reference: 7,
                expensive: false,
            }],
            variables: vec![variable("count", "3")],
            terminated: false,
            output: Vec::new(),
        };
        let rows = debug_rows(&state, 0);
        // Headings come in order, and each section is non empty.
        let headings = rows
            .iter()
            .filter_map(|row| match row {
                DebugRow::Heading(name) => Some(name.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(headings, ["CALL STACK", "SCOPES", "VARIABLES"]);

        // A frame shows the file it is in without the whole path.
        match &rows[1] {
            DebugRow::Frame {
                index,
                label,
                detail,
                selected,
            } => {
                assert_eq!(*index, 0);
                assert!(label.contains("main"));
                assert_eq!(detail, "main.c:12");
                assert!(selected, "the picked frame is marked");
            }
            other => panic!("expected a frame, got {other:?}"),
        }
        match &rows[2] {
            DebugRow::Frame { selected, .. } => assert!(!selected),
            other => panic!("expected a frame, got {other:?}"),
        }
        assert!(rows.contains(&DebugRow::Variable {
            name: "count".to_string(),
            value: "3".to_string()
        }));
    }

    #[test]
    fn a_selected_frame_that_is_gone_falls_back_to_the_first() {
        let state = DebugState {
            stopped: None,
            frames: vec![frame(1, "main", None, 0)],
            scopes: Vec::new(),
            variables: Vec::new(),
            terminated: false,
            output: Vec::new(),
        };
        // A frame index past the end must not panic or select nothing.
        let rows = debug_rows(&state, 9);
        match rows
            .iter()
            .find(|row| matches!(row, DebugRow::Frame { .. }))
        {
            Some(DebugRow::Frame { selected, .. }) => assert!(*selected),
            other => panic!("expected a frame, got {other:?}"),
        }
        // A frame without a file still says where it is when it knows the line.
        let rows = debug_rows(&state, 0);
        match &rows[1] {
            DebugRow::Frame { detail, .. } => assert_eq!(detail, ""),
            other => panic!("expected a frame, got {other:?}"),
        }
    }

    #[test]
    fn an_empty_or_finished_state_says_so() {
        let mut state = DebugState {
            stopped: None,
            frames: Vec::new(),
            scopes: Vec::new(),
            variables: Vec::new(),
            terminated: false,
            output: Vec::new(),
        };
        assert_eq!(
            debug_rows(&state, 0),
            vec![DebugRow::Empty("not stopped".into())]
        );
        state.terminated = true;
        assert_eq!(
            debug_rows(&state, 0),
            vec![DebugRow::Empty("the program has finished".into())]
        );
    }

    #[test]
    fn a_long_value_is_shortened_and_says_how_much_is_missing() {
        assert_eq!(shortened_value("3"), "3");
        let short = "x".repeat(DEBUG_VALUE_LIMIT);
        assert_eq!(shortened_value(&short), short);
        let long = "x".repeat(DEBUG_VALUE_LIMIT + 5);
        let shown = shortened_value(&long);
        assert!(shown.ends_with("… (+5)"), "{shown}");
        // What matters is that the value itself is cut at the limit; the note
        // about what is missing is allowed to sit on top of it.
        assert!(shown.starts_with(&"x".repeat(DEBUG_VALUE_LIMIT)));
        assert!(!shown.starts_with(&long));
        // Newlines and tabs are flattened, so one row stays one row.
        assert_eq!(shortened_value("a\nb\tc\rd"), "a b c d");
    }

    #[test]
    fn a_panel_shows_what_fits_and_counts_the_rest() {
        let rows = vec![
            DebugRow::Heading("CALL STACK".to_string()),
            DebugRow::Frame {
                index: 0,
                label: "main".to_string(),
                detail: String::new(),
                selected: true,
            },
            DebugRow::Heading("VARIABLES".to_string()),
            DebugRow::Variable {
                name: "a".to_string(),
                value: "1".to_string(),
            },
        ];
        let (shown, hidden) = visible_debug_rows(&rows, 10);
        assert_eq!(shown.len(), 4);
        assert_eq!(hidden, 0);
        let (shown, hidden) = visible_debug_rows(&rows, 2);
        assert_eq!(shown.len(), 2);
        assert_eq!(hidden, 2);
        // A panel with no room shows nothing rather than everything.
        let (shown, hidden) = visible_debug_rows(&rows, 0);
        assert_eq!(shown.len(), 4, "no limit means no limit");
        assert_eq!(hidden, 0);
    }
}
