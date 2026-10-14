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
pub fn set_breakpoints_arguments(path: &str, lines: &[usize]) -> String {
    let path = path.replace('\\', "\\\\").replace('"', "\\\"");
    let breakpoints = lines
        .iter()
        .map(|line| format!("{{\"line\":{}}}", line + 1))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{\"source\":{{\"path\":\"{path}\"}},\"breakpoints\":[{breakpoints}]}}")
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
        breakpoints: &[(String, Vec<usize>)],
    ) -> Result<(), String> {
        self.request(PendingRequest::Initialize, &initialize_arguments("tc-ide"))?;
        for (path, lines) in breakpoints {
            if lines.is_empty() {
                continue;
            }
            self.request(
                PendingRequest::SetBreakpoints,
                &set_breakpoints_arguments(path, lines),
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
    use crate::lsp_core::executable_available;

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
        let breakpoints = set_breakpoints_arguments("/p/a.rs", &[2, 9]);
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
        if !executable_available(python) {
            return;
        }
        let directory = std::env::temp_dir().join("tc-ide-dap-test");
        let _ = std::fs::create_dir_all(&directory);
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
                &[("/tmp/demo.py".to_string(), vec![2])],
            )
            .expect("launch");

        // Drive the session until it has stopped, then collect its state.
        let mut saw_stop = false;
        for _ in 0..1500 {
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
        for _ in 0..1500 {
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
}
