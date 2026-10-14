//! Shared language server protocol layer.
//!
//! A language server client is mostly framing, request building and response
//! parsing; only the transport is platform specific. Keeping the protocol here
//! means Linux `cargo test` covers message framing, the requests TC-IDE sends
//! and the diagnostics, completion and hover payloads it understands.
#![allow(dead_code)] // The backends adopt the pieces they need as they migrate.

use std::fmt::Write as _;

/// Language servers TC-IDE knows how to start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServerKind {
    RustAnalyzer,
    Clangd,
    Pyright,
    TypeScript,
    Gopls,
    EclipseJdt,
}

/// A server program and the arguments it needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerSpec {
    pub kind: ServerKind,
    pub program: String,
    pub arguments: Vec<String>,
    /// Command the UI can run to report whether the server is really installed.
    pub probe_command: String,
}

impl ServerKind {
    /// Server used for a file, if TC-IDE supports the language.
    pub fn for_file(name: &str) -> Option<ServerKind> {
        let extension = name.rsplit_once('.').map(|(_, value)| value)?;
        match extension.to_ascii_lowercase().as_str() {
            "rs" => Some(ServerKind::RustAnalyzer),
            "c" | "h" | "cc" | "cpp" | "cxx" | "hpp" => Some(ServerKind::Clangd),
            "py" | "pyi" => Some(ServerKind::Pyright),
            "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" => Some(ServerKind::TypeScript),
            "go" => Some(ServerKind::Gopls),
            "java" => Some(ServerKind::EclipseJdt),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ServerKind::RustAnalyzer => "rust-analyzer",
            ServerKind::Clangd => "clangd",
            ServerKind::Pyright => "basedpyright",
            ServerKind::TypeScript => "typescript-language-server",
            ServerKind::Gopls => "gopls",
            ServerKind::EclipseJdt => "jdtls",
        }
    }
}

/// Server program and arguments for a language, when one is known.
pub fn server_spec(kind: ServerKind) -> ServerSpec {
    let (program, arguments) = match kind {
        ServerKind::RustAnalyzer => ("rust-analyzer", vec![]),
        ServerKind::Clangd => ("clangd", vec!["--background-index".to_string()]),
        ServerKind::Pyright => ("basedpyright-langserver", vec!["--stdio".to_string()]),
        ServerKind::TypeScript => ("typescript-language-server", vec!["--stdio".to_string()]),
        ServerKind::Gopls => ("gopls", vec!["serve".to_string()]),
        ServerKind::EclipseJdt => ("jdtls", vec![]),
    };
    ServerSpec {
        kind,
        program: program.to_string(),
        arguments,
        probe_command: format!("{} --version", kind.label()),
    }
}

/// Server for a file, with its command line.
pub fn server_for_file(name: &str) -> Option<ServerSpec> {
    ServerKind::for_file(name).map(server_spec)
}

/// Frame a JSON-RPC message with its `Content-Length` header.
pub fn encode_message(json: &str) -> String {
    format!("Content-Length: {}\r\n\r\n{json}", json.len())
}

/// Split every complete message out of a receive buffer.
///
/// Returns the messages and whatever is left of an incomplete frame, so a
/// server that splits a header across packets can never corrupt the stream.
pub fn decode_messages(buffer: &str) -> (Vec<String>, String) {
    let mut messages = Vec::new();
    let mut rest = buffer.to_string();
    while let Some(header_end) = rest.find("\r\n\r\n") {
        let header = &rest[..header_end];
        let Some(length) = header.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.trim().eq_ignore_ascii_case("content-length") {
                value.trim().parse::<usize>().ok()
            } else {
                None
            }
        }) else {
            // Unusable header: drop it instead of looping forever.
            rest = rest[header_end + 4..].to_string();
            continue;
        };
        let body_start = header_end + 4;
        if rest.len() < body_start + length {
            break;
        }
        let body = rest[body_start..body_start + length].to_string();
        rest = rest[body_start + length..].to_string();
        messages.push(body);
    }
    (messages, rest)
}

/// Escape a string for a JSON document.
fn escape(value: &str) -> String {
    let mut result = String::new();
    for character in value.chars() {
        match character {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            value if (value as u32) < 0x20 => {
                let _ = write!(result, "\\u{:04x}", value as u32);
            }
            value => result.push(value),
        }
    }
    result
}

/// File URI a server expects for a path.
pub fn file_uri(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    if normalized.starts_with('/') {
        format!("file://{normalized}")
    } else if normalized.len() > 2 && normalized.as_bytes()[1] == b':' {
        format!("file:///{}", normalized)
    } else {
        format!("file:///{normalized}")
    }
}

/// `initialize` request.
pub fn initialize_request(id: u64, root: &str) -> String {
    format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":{id},\"method\":\"initialize\",\"params\":{{\"processId\":null,\"rootUri\":\"{}\",\"capabilities\":{{\"textDocument\":{{\"completion\":{{\"completionItem\":{{\"snippetSupport\":true}}}},\"publishDiagnostics\":{{}},\"hover\":{{}},\"definition\":{{}},\"references\":{{}},\"rename\":{{}}}}}}}}}}",
        escape(&file_uri(root))
    )
}

/// `initialized` notification.
pub fn initialized_notification() -> String {
    "{\"jsonrpc\":\"2.0\",\"method\":\"initialized\",\"params\":{}}".to_string()
}

/// `textDocument/didOpen` notification.
pub fn did_open_notification(path: &str, language_id: &str, text: &str) -> String {
    format!(
        "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{}\",\"languageId\":\"{}\",\"version\":1,\"text\":\"{}\"}}}}}}",
        escape(&file_uri(path)),
        escape(language_id),
        escape(text)
    )
}

/// `textDocument/didChange` notification for a full document sync.
pub fn did_change_notification(path: &str, version: u64, text: &str) -> String {
    format!(
        "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didChange\",\"params\":{{\"textDocument\":{{\"uri\":\"{}\",\"version\":{version}}},\"contentChanges\":[{{\"text\":\"{}\"}}]}}}}",
        escape(&file_uri(path)),
        escape(text)
    )
}

/// Request for completion, hover, definition, references or rename.
pub fn position_request(id: u64, method: &str, path: &str, line: u32, character: u32) -> String {
    format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":{id},\"method\":\"{method}\",\"params\":{{\"textDocument\":{{\"uri\":\"{}\"}},\"position\":{{\"line\":{line},\"character\":{character}}}}}}}",
        escape(&file_uri(path))
    )
}

/// Rename request, which needs a new name as well as a position.
pub fn rename_request(id: u64, path: &str, line: u32, character: u32, new_name: &str) -> String {
    format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":{id},\"method\":\"textDocument/rename\",\"params\":{{\"textDocument\":{{\"uri\":\"{}\"}},\"position\":{{\"line\":{line},\"character\":{character}}},\"newName\":\"{}\"}}}}",
        escape(&file_uri(path)),
        escape(new_name)
    )
}

/// `shutdown` request.
pub fn shutdown_request(id: u64) -> String {
    format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"method\":\"shutdown\",\"params\":null}}")
}

/// `exit` notification.
pub fn exit_notification() -> String {
    "{\"jsonrpc\":\"2.0\",\"method\":\"exit\",\"params\":null}".to_string()
}

/// Byte level frame reader.
///
/// Reading must tolerate a multi byte character or a header split across two
/// sockets reads, so frames are assembled from bytes and only decoded once a
/// complete body has arrived.
#[derive(Default)]
pub struct FrameReader {
    buffer: Vec<u8>,
}

impl FrameReader {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed received bytes and take every complete message body.
    pub fn push(&mut self, bytes: &[u8]) -> Vec<String> {
        self.buffer.extend_from_slice(bytes);
        let mut messages = Vec::new();
        while let Some(header_end) = find_header_end(&self.buffer) {
            let header = String::from_utf8_lossy(&self.buffer[..header_end]).to_string();
            let Some(length) = content_length(&header) else {
                // Unusable header: drop it rather than stalling forever.
                self.buffer.drain(..header_end + 4);
                continue;
            };
            let body_start = header_end + 4;
            if self.buffer.len() < body_start + length {
                break;
            }
            let body =
                String::from_utf8_lossy(&self.buffer[body_start..body_start + length]).to_string();
            self.buffer.drain(..body_start + length);
            messages.push(body);
        }
        messages
    }

    /// Bytes of an incomplete frame still waiting for more input.
    pub fn pending(&self) -> usize {
        self.buffer.len()
    }
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn content_length(header: &str) -> Option<usize> {
    header.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name.trim().eq_ignore_ascii_case("content-length") {
            value.trim().parse::<usize>().ok()
        } else {
            None
        }
    })
}

/// What kind of message a server sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageKind {
    /// A response to one of our requests.
    Response,
    /// A notification we only have to observe.
    Notification,
    /// A request from the server that we must answer.
    ServerRequest,
}

/// Classify an incoming message by its `id` and `method` fields.
pub fn message_kind(text: &str) -> MessageKind {
    let has_method = text.contains("\"method\"");
    let has_id = text.contains("\"id\"");
    match (has_method, has_id) {
        (true, true) => MessageKind::ServerRequest,
        (true, false) => MessageKind::Notification,
        _ => MessageKind::Response,
    }
}

/// Method of a message, when it has one.
pub fn message_method(text: &str) -> Option<String> {
    json_string(text, "method")
}

/// Numeric id of a message, when it has one.
pub fn message_id(text: &str) -> Option<u64> {
    json_number(text, "id")
}

/// Replies the client must send to keep a server running.
///
/// Servers ask for configuration or capabilities and wait for an answer; a
/// client that stays silent makes them look broken.
pub fn automatic_reply(request: &str) -> Option<String> {
    let method = message_method(request)?;
    let id = message_id(request)?;
    let result = match method.as_str() {
        "workspace/configuration" => {
            let items = json_objects(request, "items");
            let values = items
                .iter()
                .map(|_| "null".to_string())
                .collect::<Vec<_>>()
                .join(",");
            format!("[{values}]")
        }
        "workspace/workspaceFolders" => "null".to_string(),
        "client/registerCapability"
        | "client/unregisterCapability"
        | "window/workDoneProgress/create"
        | "window/showMessageRequest" => "null".to_string(),
        _ => "null".to_string(),
    };
    Some(format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":{id},\"result\":{result}}}"
    ))
}

/// `textDocument/didClose` notification.
pub fn did_close_notification(path: &str) -> String {
    format!(
        "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didClose\",\"params\":{{\"textDocument\":{{\"uri\":\"{}\"}}}}}}",
        escape(&file_uri(path))
    )
}

/// Language id a server expects for a file.
pub fn language_id(name: &str) -> &'static str {
    let extension = name.rsplit_once('.').map(|(_, value)| value).unwrap_or("");
    match extension.to_ascii_lowercase().as_str() {
        "rs" => "rust",
        "c" | "h" => "c",
        "cc" | "cpp" | "cxx" | "hpp" => "cpp",
        "py" | "pyi" => "python",
        "ts" => "typescript",
        "tsx" => "typescriptreact",
        "js" | "mjs" | "cjs" => "javascript",
        "jsx" => "javascriptreact",
        "go" => "go",
        "java" => "java",
        "json" => "json",
        "md" => "markdown",
        _ => "plaintext",
    }
}

/// Whether a program is really available on `PATH`.
///
/// The extension page uses this so an install state reflects a real executable
/// instead of a stored flag.
pub fn executable_available(program: &str) -> bool {
    if program.is_empty() {
        return false;
    }
    let candidate = std::path::Path::new(program);
    if candidate.is_absolute() {
        return candidate.is_file();
    }
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    let extensions: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".EXE;.CMD;.BAT".to_string())
            .split(';')
            .map(|value| value.to_ascii_lowercase())
            .collect()
    } else {
        Vec::new()
    };
    for directory in std::env::split_paths(&path) {
        let direct = directory.join(program);
        if direct.is_file() {
            return true;
        }
        for extension in &extensions {
            let mut name = program.to_string();
            name.push_str(extension);
            if directory.join(name).is_file() {
                return true;
            }
        }
    }
    false
}

/// Server for a file together with whether it is installed.
pub fn server_status(name: &str) -> Option<(ServerSpec, bool)> {
    let spec = server_for_file(name)?;
    let installed = executable_available(&spec.program);
    Some((spec, installed))
}

/// A running language server with piped standard input and output.
///
/// Starting a server, framing requests and collecting messages is the same on
/// every platform, so the client lives in shared code; the backends only decide
/// when to start it and what to do with the answers.
pub struct LanguageServer {
    child: std::process::Child,
    input: Option<std::process::ChildStdin>,
    messages: std::sync::mpsc::Receiver<String>,
    errors: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    next_id: u64,
    root: String,
    opened: Vec<String>,
    version: u64,
    spec: ServerSpec,
}

impl LanguageServer {
    /// Start a server in a project directory.
    pub fn start(spec: &ServerSpec, root: &std::path::Path) -> Result<Self, String> {
        use std::process::{Command, Stdio};
        let mut command = Command::new(&spec.program);
        command
            .args(&spec.arguments)
            .current_dir(root)
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
            .ok_or_else(|| "the server has no standard output".to_string())?;
        let errors = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let (sender, messages) = std::sync::mpsc::channel();
        let reader_errors = std::sync::Arc::clone(&errors);
        std::thread::spawn(move || {
            use std::io::Read;
            let mut stdout = stdout;
            let mut reader = FrameReader::new();
            let mut buffer = [0u8; 8192];
            loop {
                match stdout.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(read) => {
                        for message in reader.push(&buffer[..read]) {
                            if sender.send(message).is_err() {
                                return;
                            }
                        }
                    }
                }
            }
            let _ = reader_errors; // the thread ends with the pipe
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
            next_id: 1,
            root: file_uri(&root.to_string_lossy()),
            opened: Vec::new(),
            version: 1,
            spec: spec.clone(),
        })
    }

    pub fn spec(&self) -> &ServerSpec {
        &self.spec
    }

    pub fn is_running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Lines the server wrote to standard error, for the status bar.
    pub fn stderr_lines(&self) -> Vec<String> {
        self.errors
            .lock()
            .map(|errors| errors.clone())
            .unwrap_or_default()
    }

    fn send(&mut self, message: String) -> Result<(), String> {
        use std::io::Write;
        let Some(input) = self.input.as_mut() else {
            return Err("the server input is closed".into());
        };
        input
            .write_all(encode_message(&message).as_bytes())
            .and_then(|()| input.flush())
            .map_err(|error| format!("could not reach the language server: {error}"))
    }

    fn allocate_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Send `initialize` and return its request id.
    pub fn initialize(&mut self) -> Result<u64, String> {
        let id = self.allocate_id();
        self.send(initialize_request(id, &self.root))?;
        Ok(id)
    }

    /// Send `initialized`, which servers require before serving requests.
    pub fn notify_initialized(&mut self) -> Result<(), String> {
        self.send(initialized_notification())
    }

    /// Tell the server about an opened document and remember it.
    pub fn open_document(&mut self, path: &str, text: &str) -> Result<(), String> {
        let language = language_id(path);
        self.send(did_open_notification(path, language, text))?;
        if !self.opened.iter().any(|opened| opened == path) {
            self.opened.push(path.to_string());
        }
        Ok(())
    }

    /// Send the new document text.
    pub fn change_document(&mut self, path: &str, text: &str) -> Result<(), String> {
        self.version += 1;
        self.send(did_change_notification(path, self.version, text))
    }

    /// Forget a document and tell the server.
    pub fn close_document(&mut self, path: &str) -> Result<(), String> {
        self.opened.retain(|opened| opened != path);
        self.send(did_close_notification(path))
    }

    /// Request completion, hover, definition, references or rename.
    pub fn position_request(
        &mut self,
        method: &str,
        path: &str,
        line: u32,
        character: u32,
    ) -> Result<u64, String> {
        let id = self.allocate_id();
        self.send(position_request(id, method, path, line, character))?;
        Ok(id)
    }

    pub fn rename(
        &mut self,
        path: &str,
        line: u32,
        character: u32,
        new_name: &str,
    ) -> Result<u64, String> {
        let id = self.allocate_id();
        self.send(rename_request(id, path, line, character, new_name))?;
        Ok(id)
    }

    /// Take the next message without blocking.
    pub fn try_message(&mut self) -> Option<String> {
        self.messages.try_recv().ok()
    }

    /// Answer the server when it needs an answer, and report whether it did.
    pub fn answer_automatically(&mut self, message: &str) -> bool {
        if message_kind(message) != MessageKind::ServerRequest {
            return false;
        }
        match automatic_reply(message) {
            Some(reply) => self.send(reply).is_ok(),
            None => false,
        }
    }

    /// Ask the server to stop and wait briefly for it to exit.
    pub fn shutdown(&mut self) {
        let id = self.allocate_id();
        let _ = self.send(shutdown_request(id));
        let _ = self.send(exit_notification());
        self.input = None;
        for _ in 0..20 {
            if !self.is_running() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
    }
}

impl Drop for LanguageServer {
    fn drop(&mut self) {
        self.shutdown();
        if self.is_running() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}

/// Severity of a diagnostic, mapped to something the UI can colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

impl DiagnosticSeverity {
    fn from_number(value: u64) -> Self {
        match value {
            1 => DiagnosticSeverity::Error,
            2 => DiagnosticSeverity::Warning,
            3 => DiagnosticSeverity::Information,
            _ => DiagnosticSeverity::Hint,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            DiagnosticSeverity::Error => "error",
            DiagnosticSeverity::Warning => "warning",
            DiagnosticSeverity::Information => "info",
            DiagnosticSeverity::Hint => "hint",
        }
    }
}

/// One diagnostic reported by a server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub path: String,
    pub line: u32,
    pub column: u32,
    pub severity: DiagnosticSeverity,
    pub message: String,
}

impl Diagnostic {
    /// Text the UI shows for this diagnostic.
    pub fn line_text(&self) -> String {
        format!(
            "{}:{}:{}: {}: {}",
            self.path,
            self.line + 1,
            self.column + 1,
            self.severity.label(),
            self.message
        )
    }
}

/// Extract every balanced `{...}` object that follows `key` in an array.
pub fn json_objects(text: &str, key: &str) -> Vec<String> {
    let needle = format!("\"{key}\"");
    let Some(position) = text.find(&needle) else {
        return Vec::new();
    };
    let rest = &text[position + needle.len()..];
    let mut objects = Vec::new();
    let mut depth = 0usize;
    let mut start = None;
    let mut in_string = false;
    let mut escaped = false;
    for (index, character) in rest.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }
        match character {
            '"' => in_string = true,
            '{' => {
                if depth == 0 {
                    start = Some(index);
                }
                depth += 1;
            }
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    if let Some(start) = start.take() {
                        objects.push(rest[start..=index].to_string());
                    }
                }
            }
            ']' if depth == 0 && !objects.is_empty() => break,
            _ => {}
        }
    }
    objects
}

/// First string value of a key inside a JSON document.
pub fn json_string(text: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let position = text.find(&needle)?;
    let rest = text[position + needle.len()..].trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let mut value = String::new();
    let mut escaped = false;
    for character in rest.chars() {
        if escaped {
            match character {
                'n' => value.push('\n'),
                'r' => value.push('\r'),
                't' => value.push('\t'),
                other => value.push(other),
            }
            escaped = false;
            continue;
        }
        match character {
            '\\' => escaped = true,
            '"' => return Some(value),
            other => value.push(other),
        }
    }
    Some(value)
}

/// First number value of a key inside a JSON document.
pub fn json_number(text: &str, key: &str) -> Option<u64> {
    let needle = format!("\"{key}\"");
    let position = text.find(&needle)?;
    let rest = text[position + needle.len()..].trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    let digits = rest
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect::<String>();
    digits.parse().ok()
}

/// Diagnostics from a `textDocument/publishDiagnostics` notification.
pub fn parse_diagnostics(text: &str) -> Vec<Diagnostic> {
    let path = json_string(text, "uri")
        .map(|uri| uri_to_path(&uri))
        .unwrap_or_default();
    json_objects(text, "diagnostics")
        .into_iter()
        .map(|object| {
            let severity = json_number(&object, "severity")
                .map(DiagnosticSeverity::from_number)
                .unwrap_or(DiagnosticSeverity::Error);
            Diagnostic {
                path: path.clone(),
                line: json_number(&object, "line").unwrap_or(0) as u32,
                column: json_number(&object, "character").unwrap_or(0) as u32,
                severity,
                message: json_string(&object, "message").unwrap_or_default(),
            }
        })
        .collect()
}

/// Turn a `file://` URI back into a path.
pub fn uri_to_path(uri: &str) -> String {
    let rest = uri.strip_prefix("file://").unwrap_or(uri);
    let rest = rest.strip_prefix('/').unwrap_or(rest);
    // Windows URIs look like `/C:/dir/file.rs`, POSIX ones like `dir/file.rs`.
    if rest.len() > 1 && rest.as_bytes()[1] == b':' {
        rest.to_string()
    } else if uri.starts_with("file:///") {
        format!("/{rest}")
    } else {
        rest.to_string()
    }
}

/// Completion labels and details from a completion response.
pub fn parse_completions(text: &str) -> Vec<(String, String)> {
    let items = if text.contains("\"items\"") {
        json_objects(text, "items")
    } else {
        json_objects(text, "result")
    };
    items
        .into_iter()
        .filter_map(|item| {
            let label = json_string(&item, "label")?;
            let detail = json_string(&item, "detail").unwrap_or_default();
            Some((label, detail))
        })
        .collect()
}

/// Hover documentation from a hover response.
pub fn parse_hover(text: &str) -> Option<String> {
    if let Some(value) = json_string(text, "value") {
        if !value.is_empty() {
            return Some(value);
        }
    }
    let contents = json_objects(text, "contents");
    contents
        .iter()
        .find_map(|object| json_string(object, "value"))
}

/// Locations from a definition or references response.
pub fn parse_locations(text: &str) -> Vec<(String, u32, u32)> {
    let locations = if text.contains("\"locations\"") {
        json_objects(text, "locations")
    } else {
        json_objects(text, "result")
    };
    locations
        .into_iter()
        .filter_map(|location| {
            let uri = json_string(&location, "uri")?;
            Some((
                uri_to_path(&uri),
                json_number(&location, "line").unwrap_or(0) as u32,
                json_number(&location, "character").unwrap_or(0) as u32,
            ))
        })
        .collect()
}

/// Text edits from a rename response, as `(path, line, column, new text)`.
pub fn parse_rename_edits(text: &str) -> Vec<(String, u32, u32, String)> {
    let mut edits = Vec::new();
    for (path, object) in document_changes(text) {
        for edit in json_objects(&object, "edits") {
            let line = json_number(&edit, "line").unwrap_or(0) as u32;
            let column = json_number(&edit, "character").unwrap_or(0) as u32;
            let new_text = json_string(&edit, "newText").unwrap_or_default();
            edits.push((path.clone(), line, column, new_text));
        }
    }
    edits
}

/// `(path, object)` pairs of a `WorkspaceEdit` style response.
///
/// Handles both shapes a server may return: a `changes` map keyed by file URI
/// and a `documentChanges` array of `textDocument` entries.
fn document_changes(text: &str) -> Vec<(String, String)> {
    let mut changes = Vec::new();
    if let Some(position) = text.find("\"changes\"") {
        let rest = &text[position..];
        let mut search = 0;
        while let Some(found) = rest[search..].find("\"file://") {
            let start = search + found + 1;
            let Some(end_of_uri) = rest[start..].find('"') else {
                break;
            };
            let uri = rest[start..start + end_of_uri].to_string();
            if let Some(object) = balanced_object(&rest[start + end_of_uri..]) {
                changes.push((uri_to_path(&uri), object));
            }
            search = start + end_of_uri + 1;
        }
    }
    if changes.is_empty() {
        for object in json_objects(text, "documentChanges") {
            if let Some(uri) = json_string(&object, "uri") {
                changes.push((uri_to_path(&uri), object));
            }
        }
    }
    changes
}

/// First balanced `{...}` object inside a string.
fn balanced_object(text: &str) -> Option<String> {
    let start = text.find('{')?;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (index, character) in text[start..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }
        match character {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(text[start..=start + index].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_are_framed_with_a_content_length_header() {
        let framed = encode_message("{\"a\":1}");
        assert_eq!(framed, "Content-Length: 7\r\n\r\n{\"a\":1}");
    }

    #[test]
    fn decoding_handles_split_and_stacked_frames() {
        let (messages, rest) = decode_messages("Content-Length: 7\r\n\r\n{\"a\":1}");
        assert_eq!(messages, ["{\"a\":1}"]);
        assert!(rest.is_empty());

        // A header split across packets must not be treated as a message.
        let (messages, rest) = decode_messages("Content-Length: 7\r\n\r\n{\"a\"");
        assert!(messages.is_empty());
        assert!(rest.starts_with("Content-Length"));

        // Two messages in one packet, plus a partial third.
        let buffer = format!(
            "{}{}Content-Length: 99\r\n\r\npartial",
            encode_message("{\"id\":1}"),
            encode_message("{\"id\":2}")
        );
        let (messages, rest) = decode_messages(&buffer);
        assert_eq!(messages, ["{\"id\":1}", "{\"id\":2}"]);
        assert_eq!(rest, "Content-Length: 99\r\n\r\npartial");

        let (messages, rest) = decode_messages("nonsense\r\n\r\n{}");
        assert!(messages.is_empty());
        assert_eq!(rest, "{}");
    }

    #[test]
    fn servers_are_chosen_per_language() {
        assert_eq!(
            ServerKind::for_file("main.rs"),
            Some(ServerKind::RustAnalyzer)
        );
        assert_eq!(ServerKind::for_file("api.cpp"), Some(ServerKind::Clangd));
        assert_eq!(ServerKind::for_file("app.py"), Some(ServerKind::Pyright));
        assert_eq!(ServerKind::for_file("ui.tsx"), Some(ServerKind::TypeScript));
        assert_eq!(ServerKind::for_file("main.go"), Some(ServerKind::Gopls));
        assert_eq!(
            ServerKind::for_file("Main.java"),
            Some(ServerKind::EclipseJdt)
        );
        assert_eq!(ServerKind::for_file("notes.md"), None);

        let spec = server_for_file("main.rs").expect("spec");
        assert_eq!(spec.program, "rust-analyzer");
        assert!(spec.probe_command.starts_with("rust-analyzer --version"));
        let clangd = server_for_file("main.cpp").expect("spec");
        assert_eq!(clangd.program, "clangd");
        assert!(clangd.arguments.contains(&"--background-index".to_string()));
    }

    #[test]
    fn requests_carry_paths_positions_and_versions() {
        let initialize = initialize_request(1, "/home/dev/project");
        assert!(initialize.contains("\"id\":1"));
        assert!(initialize.contains("file:///home/dev/project"));
        assert!(initialize.contains("\"completion\""));
        assert_eq!(
            initialized_notification(),
            "{\"jsonrpc\":\"2.0\",\"method\":\"initialized\",\"params\":{}}"
        );

        let open = did_open_notification("/tmp/app.py", "python", "print(\"hi\")\n");
        assert!(open.contains("file:///tmp/app.py"));
        assert!(open.contains("\"languageId\":\"python\""));
        assert!(open.contains("print(\\\"hi\\\")"));

        let change = did_change_notification("C:\\src\\main.rs", 4, "fn main() {}");
        assert!(change.contains("\"version\":4"));
        assert!(change.contains("\"text\":\"fn main() {}\""));

        let definition = position_request(7, "textDocument/definition", "/a/b.rs", 12, 4);
        assert!(definition.contains("\"method\":\"textDocument/definition\""));
        assert!(definition.contains("\"line\":12"));
        assert!(definition.contains("\"character\":4"));
        let rename = rename_request(9, "/a/b.rs", 1, 2, "newName");
        assert!(rename.contains("\"newName\":\"newName\""));
        assert!(shutdown_request(11).contains("\"method\":\"shutdown\""));
        assert!(exit_notification().contains("\"method\":\"exit\""));
    }

    #[test]
    fn uris_round_trip_for_both_path_styles() {
        assert_eq!(file_uri("/home/dev/a.rs"), "file:///home/dev/a.rs");
        assert_eq!(uri_to_path("file:///home/dev/a.rs"), "/home/dev/a.rs");
        assert_eq!(file_uri("C:\\src\\a.rs"), "file:///C:/src/a.rs");
        assert_eq!(uri_to_path("file:///C:/src/a.rs"), "C:/src/a.rs");
    }

    #[test]
    fn diagnostics_are_parsed_with_severity_and_position() {
        let message = "{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/publishDiagnostics\",\"params\":{\"uri\":\"file:///p/src/main.rs\",\"diagnostics\":[{\"range\":{\"start\":{\"line\":3,\"character\":8},\"end\":{\"line\":3,\"character\":9}},\"severity\":1,\"message\":\"cannot find value `x`\"},{\"range\":{\"start\":{\"line\":9,\"character\":0},\"end\":{\"line\":9,\"character\":1}},\"severity\":2,\"message\":\"unused import\"}]}}";
        let diagnostics = parse_diagnostics(message);
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].path, "/p/src/main.rs");
        assert_eq!(diagnostics[0].line, 3);
        assert_eq!(diagnostics[0].column, 8);
        assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Error);
        assert_eq!(diagnostics[0].message, "cannot find value `x`");
        assert_eq!(diagnostics[1].severity, DiagnosticSeverity::Warning);
        assert_eq!(
            diagnostics[0].line_text(),
            "/p/src/main.rs:4:9: error: cannot find value `x`"
        );
        assert!(parse_diagnostics("{\"params\":{}}").is_empty());
    }

    #[test]
    fn completion_hover_and_locations_are_extracted() {
        let completion = "{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"isIncomplete\":false,\"items\":[{\"label\":\"println!\",\"detail\":\"macro\"},{\"label\":\"print\",\"detail\":\"fn\"}]}}";
        let items = parse_completions(completion);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0], ("println!".to_string(), "macro".to_string()));

        let hover = "{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"contents\":{\"kind\":\"markdown\",\"value\":\"`fn main()`\\n\\nentry point\"}}}";
        let text = parse_hover(hover).expect("hover");
        assert!(text.contains("entry point"));

        let definition = "{\"jsonrpc\":\"2.0\",\"id\":3,\"result\":[{\"uri\":\"file:///p/src/lib.rs\",\"range\":{\"start\":{\"line\":4,\"character\":2},\"end\":{\"line\":4,\"character\":6}}}]}";
        let locations = parse_locations(definition);
        assert_eq!(locations, [("/p/src/lib.rs".to_string(), 4, 2)]);
        let references = "{\"result\":{\"locations\":[{\"uri\":\"file:///p/a.rs\",\"range\":{\"start\":{\"line\":1,\"character\":0},\"end\":{}}}]}}";
        assert_eq!(parse_locations(references).len(), 1);
    }

    #[test]
    fn rename_edits_are_extracted_per_file() {
        let response = "{\"jsonrpc\":\"2.0\",\"id\":4,\"result\":{\"changes\":{\"file:///p/src/main.rs\":{\"edits\":[{\"range\":{\"start\":{\"line\":1,\"character\":4},\"end\":{\"line\":1,\"character\":8}},\"newText\":\"renamed\"}]},\"file:///p/src/lib.rs\":{\"edits\":[{\"range\":{\"start\":{\"line\":8,\"character\":1},\"end\":{}},\"newText\":\"renamed\"}]}}}}";
        let edits = parse_rename_edits(response);
        assert_eq!(edits.len(), 2);
        assert!(edits
            .iter()
            .any(|(path, line, column, text)| path == "/p/src/main.rs"
                && *line == 1
                && *column == 4
                && text == "renamed"));
        assert!(edits.iter().any(|(path, ..)| path == "/p/src/lib.rs"));

        // The `documentChanges` shape is understood too.
        let array_shape = "{\"result\":{\"documentChanges\":[{\"textDocument\":{\"uri\":\"file:///p/a.rs\",\"version\":2},\"edits\":[{\"range\":{\"start\":{\"line\":3,\"character\":0},\"end\":{}},\"newText\":\"renamed\"}]}]}}";
        let edits = parse_rename_edits(array_shape);
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].0, "/p/a.rs");
        assert_eq!(edits[0].1, 3);
    }

    #[test]
    fn byte_frames_survive_split_packets_and_multibyte_text() {
        let mut reader = FrameReader::new();
        let message = encode_message("{\"text\":\"你好\"}");
        let bytes = message.as_bytes();
        let split = bytes.len() - 4;
        assert!(reader.push(&bytes[..split]).is_empty());
        assert!(reader.pending() > 0);
        let messages = reader.push(&bytes[split..]);
        assert_eq!(messages, ["{\"text\":\"你好\"}"]);

        // Two frames in one read, plus a partial third.
        let mut reader = FrameReader::new();
        let buffer = format!(
            "{}{}Content-Length: 4\r\n\r\n{}",
            encode_message("{\"a\":1}"),
            encode_message("{\"b\":2}"),
            "par"
        );
        let messages = reader.push(buffer.as_bytes());
        assert_eq!(messages, ["{\"a\":1}", "{\"b\":2}"]);
        // A garbage header is dropped instead of blocking the stream.
        let mut reader = FrameReader::new();
        let messages = reader.push(b"junk\r\n\r\nContent-Length: 2\r\n\r\n{}");
        assert_eq!(messages, ["{}"]);
        assert_eq!(reader.pending(), 0);
    }

    #[test]
    fn messages_are_classified_and_client_requests_are_answered() {
        let response = "{\"jsonrpc\":\"2.0\",\"id\":3,\"result\":null}";
        assert_eq!(message_kind(response), MessageKind::Response);
        assert_eq!(message_id(response), Some(3));
        assert_eq!(message_method(response), None);

        let notification =
            "{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/publishDiagnostics\",\"params\":{}}";
        assert_eq!(message_kind(notification), MessageKind::Notification);

        let request = "{\"jsonrpc\":\"2.0\",\"id\":9,\"method\":\"workspace/configuration\",\"params\":{\"items\":[{\"section\":\"rust-analyzer\"},{\"section\":\"editor\"}]}}";
        assert_eq!(message_kind(request), MessageKind::ServerRequest);
        let reply = automatic_reply(request).expect("reply");
        assert!(reply.contains("\"id\":9"));
        assert!(reply.contains("[null,null]"));
        let unknown = automatic_reply("{\"id\":4,\"method\":\"custom/thing\"}").expect("reply");
        assert!(unknown.contains("\"result\":null"));
        assert!(automatic_reply("{\"method\":\"no/id\"}").is_none());
    }

    #[test]
    fn language_ids_and_executable_probing_are_real() {
        assert_eq!(language_id("main.rs"), "rust");
        assert_eq!(language_id("App.cpp"), "cpp");
        assert_eq!(language_id("view.tsx"), "typescriptreact");
        assert_eq!(language_id("notes.txt"), "plaintext");
        assert!(executable_available(if cfg!(windows) {
            "cmd"
        } else {
            "sh"
        }));
        assert!(!executable_available("tc-ide-definitely-missing-program"));
        assert!(!executable_available(""));
        let status = server_status("main.rs").expect("status");
        assert_eq!(status.0.program, "rust-analyzer");
        // The installation flag matches the real PATH, whatever it says.
        assert_eq!(status.1, executable_available("rust-analyzer"));
    }

    #[test]
    fn did_close_notification_names_the_document() {
        let notification = did_close_notification("/tmp/app.py");
        assert!(notification.contains("textDocument/didClose"));
        assert!(notification.contains("file:///tmp/app.py"));
    }

    #[test]
    fn a_real_server_process_is_driven_over_pipes() {
        // A tiny python language server: answers initialize, publishes one
        // diagnostic, asks for configuration (which the client must answer) and
        // answers completion.
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
    body = sys.stdin.buffer.read(length)
    return json.loads(body.decode("utf-8"))

def send(payload):
    body = json.dumps(payload).encode("utf-8")
    sys.stdout.buffer.write(b"Content-Length: %d\r\n\r\n" % len(body))
    sys.stdout.buffer.write(body)
    sys.stdout.buffer.flush()

while True:
    message = read_message()
    if message is None:
        break
    method = message.get("method")
    if method == "initialize":
        send({"jsonrpc": "2.0", "id": message["id"], "result": {"capabilities": {}}})
    elif method == "initialized":
        send({"jsonrpc": "2.0", "id": 100, "method": "workspace/configuration",
              "params": {"items": [{"section": "demo"}]}})
        send({"jsonrpc": "2.0", "method": "textDocument/publishDiagnostics",
              "params": {"uri": "file:///tmp/app.py", "diagnostics": [
                  {"range": {"start": {"line": 2, "character": 1}, "end": {"line": 2, "character": 2}},
                   "severity": 1, "message": "boom"}]}})
    elif method == "textDocument/didOpen":
        send({"jsonrpc": "2.0", "method": "textDocument/publishDiagnostics",
              "params": {"uri": "file:///tmp/app.py", "diagnostics": []}})
    elif method == "textDocument/completion":
        send({"jsonrpc": "2.0", "id": message["id"],
              "result": {"items": [{"label": "print", "detail": "builtin"}]}})
    elif method == "shutdown":
        send({"jsonrpc": "2.0", "id": message["id"], "result": None})
    elif method == "exit":
        break
"#;
        let directory = std::env::temp_dir().join("tc-ide-lsp-test");
        let _ = std::fs::create_dir_all(&directory);
        let script_path = directory.join("fake_server.py");
        std::fs::write(&script_path, script).expect("write fake server");
        let python = if cfg!(windows) { "python" } else { "python3" };
        if !executable_available(python) {
            // Without a python interpreter this test cannot spawn a server.
            return;
        }
        let spec = ServerSpec {
            kind: ServerKind::Pyright,
            program: python.to_string(),
            arguments: vec![script_path.to_string_lossy().into_owned()],
            probe_command: String::new(),
        };
        let mut server = LanguageServer::start(&spec, &directory).expect("start server");
        assert!(server.is_running());

        let initialize = server.initialize().expect("initialize");
        assert_eq!(initialize, 1);
        server.notify_initialized().expect("initialized");
        server
            .open_document("/tmp/app.py", "print(1)\n")
            .expect("didOpen");

        // Collect messages, answering any server request as the real client does.
        let mut diagnostics = 0;
        let mut collected = Vec::new();
        for _ in 0..200 {
            let Some(message) = server.try_message() else {
                std::thread::sleep(std::time::Duration::from_millis(10));
                continue;
            };
            if server.answer_automatically(&message) {
                continue;
            }
            if message_method(&message).as_deref() == Some("textDocument/publishDiagnostics") {
                diagnostics += parse_diagnostics(&message).len();
            }
            collected.push(message);
            if diagnostics >= 1 && collected.len() >= 2 {
                break;
            }
        }
        assert!(diagnostics >= 1, "no diagnostics arrived: {collected:?}");

        let id = server
            .position_request("textDocument/completion", "/tmp/app.py", 0, 5)
            .expect("completion");
        let mut completion = None;
        for _ in 0..200 {
            let Some(message) = server.try_message() else {
                std::thread::sleep(std::time::Duration::from_millis(10));
                continue;
            };
            if server.answer_automatically(&message) {
                continue;
            }
            if message_id(&message) == Some(id) {
                completion = Some(parse_completions(&message));
                break;
            }
        }
        assert_eq!(
            completion,
            Some(vec![("print".to_string(), "builtin".to_string())])
        );

        server.close_document("/tmp/app.py").expect("didClose");
        drop(server);
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn nested_json_objects_are_balanced_and_ignore_strings() {
        let objects = json_objects("{\"a\":[{\"b\":1},{\"c\":{\"d\":2}}]}", "a");
        assert_eq!(objects.len(), 2);
        assert_eq!(objects[0], "{\"b\":1}");
        assert!(objects[1].contains("\"d\":2"));
        let braces = json_objects("{\"a\":[{\"b\":\"}{\"}]}", "a");
        assert_eq!(braces, ["{\"b\":\"}{\"}"]);
        assert!(json_objects("{\"a\":[]}", "a").is_empty());
    }
}
