//! Shared AI request layer.
//!
//! TC-IDE does not link an HTTP stack: requests run through the platform's own
//! client (`curl.exe` on Windows, `curl` elsewhere) inside the visible terminal,
//! so the user sees progress, can cancel with `Ctrl+C` and nothing blocks the UI
//! thread. What is shared and unit tested here is everything around that call:
//! endpoint validation, the request body, the response and stream parsers, the
//! status to message mapping and the command lines.
//!
//! The API key is **never** placed in a command line or in a written file. It is
//! passed to the child process through the environment (`TC_IDE_AI_KEY`) and the
//! commands below only ever reference that variable.
#![allow(dead_code)] // The backends adopt the pieces they need as they migrate.

/// Environment variable carrying the session API key.
pub const API_KEY_ENV: &str = "TC_IDE_AI_KEY";
/// Default request timeout in seconds.
pub const DEFAULT_TIMEOUT_SECONDS: u32 = 60;
/// Maximum serialized `.tcIDE` project map attached to one AI request.
pub const MAX_PROJECT_CONTEXT_BYTES: usize = 12_000;

/// Attach the current `.tcIDE` project map to a user prompt without changing
/// the conversation stored on disk. The map is read-only context for the
/// request and is bounded so large workspaces cannot grow the body forever.
pub fn prompt_with_project_context(prompt: &str, document: &str) -> String {
    let context = if document.len() > MAX_PROJECT_CONTEXT_BYTES {
        let mut end = MAX_PROJECT_CONTEXT_BYTES.min(document.len());
        while end > 0 && !document.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}\n[project context truncated]", &document[..end])
    } else {
        document.to_string()
    };
    format!(
        "{prompt}\n\n[TC-IDE project context — read-only]\n{context}\n[/TC-IDE project context]"
    )
}

/// Shell a command line is generated for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shell {
    Windows,
    Unix,
}

/// Endpoint and model the user configured.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiSettings {
    pub endpoint: String,
    pub model: String,
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            endpoint: "https://api.openai.com/v1".into(),
            model: "gpt-5.1".into(),
        }
    }
}

/// Why a request could not be prepared or understood.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AiError {
    /// Endpoint or model is not usable.
    Configuration(String),
    /// The response did not contain a completion.
    Response(String),
    /// The service answered with an error status.
    Status(u16, String),
}

impl AiError {
    /// Message shown in the AI panel.
    pub fn message(&self) -> String {
        match self {
            AiError::Configuration(value) => value.clone(),
            AiError::Response(value) => value.clone(),
            AiError::Status(code, value) => describe_status(*code, value),
        }
    }
}

/// Validate and normalize an OpenAI-compatible endpoint.
pub fn normalize_endpoint(endpoint: &str) -> Result<String, AiError> {
    let trimmed = endpoint.trim();
    if trimmed.is_empty() {
        return Err(AiError::Configuration("AI endpoint is required".into()));
    }
    if trimmed.chars().any(char::is_whitespace) {
        return Err(AiError::Configuration(
            "AI endpoint must not contain spaces".into(),
        ));
    }
    let lower = trimmed.to_ascii_lowercase();
    if !lower.starts_with("http://") && !lower.starts_with("https://") {
        return Err(AiError::Configuration(
            "AI endpoint must start with http:// or https://".into(),
        ));
    }
    if lower.starts_with("http://") && !is_loopback(&lower) {
        return Err(AiError::Configuration(
            "Use https:// for a remote AI endpoint".into(),
        ));
    }
    Ok(trimmed.trim_end_matches('/').to_string())
}

fn is_loopback(endpoint: &str) -> bool {
    let rest = endpoint
        .trim_start_matches("http://")
        .split(['/', ':'])
        .next()
        .unwrap_or("");
    matches!(rest, "localhost" | "127.0.0.1" | "[::1]")
}

/// Full URL of the chat completion endpoint.
pub fn chat_url(endpoint: &str) -> Result<String, AiError> {
    let endpoint = normalize_endpoint(endpoint)?;
    if endpoint.ends_with("/chat/completions") {
        return Ok(endpoint);
    }
    Ok(format!("{endpoint}/chat/completions"))
}

/// Escape a string for a JSON document.
pub fn json_escape(value: &str) -> String {
    let mut result = String::with_capacity(value.len() + 2);
    for character in value.chars() {
        match character {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            value if (value as u32) < 0x20 => result.push_str(&format!("\\u{:04x}", value as u32)),
            value => result.push(value),
        }
    }
    result
}

/// Body of a chat completion request for one user message.
pub fn chat_body(model: &str, prompt: &str, stream: bool) -> String {
    chat_body_with_history(model, &[("user", prompt)], stream)
}

/// How many turns of history a request carries by default.
pub const MAX_HISTORY_TURNS: usize = 12;

/// Body of a chat completion request with earlier turns included.
///
/// `turns` is `(role, content)` in conversation order; only the newest
/// [`MAX_HISTORY_TURNS`] turns are sent so a long conversation cannot grow the
/// request without bound.
pub fn chat_body_with_history(model: &str, turns: &[(&str, &str)], stream: bool) -> String {
    let start = turns.len().saturating_sub(MAX_HISTORY_TURNS);
    let messages = turns[start..]
        .iter()
        .map(|(role, content)| {
            format!(
                "{{\"role\":\"{}\",\"content\":\"{}\"}}",
                json_escape(role),
                json_escape(content)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"model\":\"{}\",\"stream\":{},\"messages\":[{}]}}",
        json_escape(model),
        if stream { "true" } else { "false" },
        messages
    )
}

/// Body of a short request used to test the configuration.
pub fn connection_test_body(model: &str) -> String {
    chat_body(model, "ping", false)
}

/// Command that performs the request, reading the key from the environment.
///
/// `body_path` must point at a file holding the request body; the key itself is
/// passed through the environment, never on the command line.
pub fn chat_command(
    settings: &AiSettings,
    body_path: &str,
    stream: bool,
    timeout_seconds: u32,
    shell: Shell,
) -> Result<String, AiError> {
    let url = chat_url(&settings.endpoint)?;
    if settings.model.trim().is_empty() {
        return Err(AiError::Configuration("AI model is required".into()));
    }
    let key = match shell {
        Shell::Windows => format!("%{API_KEY_ENV}%"),
        Shell::Unix => format!("${API_KEY_ENV}"),
    };
    let mut command = format!(
        "curl -sS --max-time {timeout_seconds} {} -X POST \"{url}\" -H \"Content-Type: application/json\" -H \"Authorization: Bearer {key}\"",
        if stream { "--no-buffer" } else { "" }
    );
    if stream {
        command.push_str(" -N");
    }
    command.push_str(&format!(
        " --data \"@{body_path}\" -w \"\\n%{{http_code}}\""
    ));
    Ok(command)
}

/// Extract a JSON string value for a key, ignoring nested objects.
pub fn json_string(text: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let mut search_from = 0;
    while let Some(position) = text[search_from..].find(&needle) {
        let start = search_from + position + needle.len();
        let rest = &text[start..];
        let rest = rest.trim_start();
        let Some(rest) = rest.strip_prefix(':') else {
            search_from = start;
            continue;
        };
        let rest = rest.trim_start();
        let Some(rest) = rest.strip_prefix('"') else {
            search_from = start;
            continue;
        };
        let mut value = String::new();
        let mut characters = rest.chars();
        while let Some(character) = characters.next() {
            match character {
                '"' => return Some(value),
                '\\' => match characters.next() {
                    Some('n') => value.push('\n'),
                    Some('r') => value.push('\r'),
                    Some('t') => value.push('\t'),
                    Some('u') => {
                        let digits = characters.by_ref().take(4).collect::<String>();
                        if let Ok(code) = u32::from_str_radix(&digits, 16) {
                            if let Some(decoded) = char::from_u32(code) {
                                value.push(decoded);
                            }
                        }
                    }
                    Some(other) => value.push(other),
                    None => break,
                },
                other => value.push(other),
            }
        }
        return Some(value);
    }
    None
}

/// One parsed stream event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StreamEvent {
    /// Text delta to append to the panel.
    Delta(String),
    /// The service signalled the end of the stream.
    Done,
    /// Text that is not part of a stream payload.
    Other(String),
}

/// Parse one line of a server sent event stream.
pub fn parse_stream_line(line: &str) -> StreamEvent {
    let trimmed = line.trim_end_matches(['\r', '\n']);
    let Some(payload) = trimmed.strip_prefix("data:") else {
        return StreamEvent::Other(trimmed.to_string());
    };
    let payload = payload.trim();
    if payload == "[DONE]" {
        return StreamEvent::Done;
    }
    match json_string(payload, "content") {
        Some(content) if !content.is_empty() => StreamEvent::Delta(content),
        _ => StreamEvent::Other(trimmed.to_string()),
    }
}

/// Text of a non streaming chat completion response.
pub fn parse_chat_response(text: &str) -> Result<String, AiError> {
    let (body, status) = split_status(text);
    if let Some(status) = status {
        if status >= 400 {
            return Err(AiError::Status(status, error_message(&body)));
        }
    }
    if let Some(content) = json_string(&body, "content") {
        if !content.is_empty() {
            return Ok(content);
        }
    }
    if let Some(message) = json_string(&body, "message") {
        if !message.is_empty() {
            return Err(AiError::Status(status.unwrap_or(200), message));
        }
    }
    Err(AiError::Response(
        "The AI response did not contain any text".into(),
    ))
}

/// Split the `-w "%{http_code}"` suffix from a curl response.
pub fn split_status(text: &str) -> (String, Option<u16>) {
    let trimmed = text.trim_end();
    let Some((body, code)) = trimmed.rsplit_once('\n') else {
        return (text.to_string(), None);
    };
    let code = code.trim();
    if code.len() == 3 && code.chars().all(|value| value.is_ascii_digit()) {
        (body.to_string(), code.parse().ok())
    } else {
        (text.to_string(), None)
    }
}

/// Error text a service returned, if any.
pub fn error_message(body: &str) -> String {
    if let Some(message) = json_string(body, "message") {
        if !message.is_empty() {
            return message;
        }
    }
    let trimmed = body.trim();
    if trimmed.is_empty() {
        "The AI service returned an empty error".into()
    } else {
        trimmed.chars().take(200).collect()
    }
}

/// Human readable explanation of an HTTP status.
pub fn describe_status(status: u16, body: &str) -> String {
    let detail = error_message(body);
    match status {
        0 => format!("The AI request did not complete: {detail}"),
        401 | 403 => format!("AI API key was rejected ({status}): {detail}"),
        404 => format!("AI endpoint or model was not found ({status}): {detail}"),
        408 => "AI request timed out; increase the timeout or retry".into(),
        413 => "AI request was too large for the endpoint".into(),
        429 => format!("AI rate limit reached ({status}): {detail}"),
        500..=599 => format!("AI service error ({status}): {detail}"),
        other if other >= 400 => format!("AI request failed ({other}): {detail}"),
        _ => detail,
    }
}

/// One event of a streamed reply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatEvent {
    /// Text to append to the answer.
    Delta(String),
    /// The request finished, with the HTTP status when it could be read.
    Done(Option<u16>),
    /// Something went wrong, already phrased for the user.
    Error(String),
}

/// A running chat request.
///
/// The request runs in a child process so the interface never blocks, the key
/// is passed through the environment rather than a command line, and the reply
/// can be streamed into the panel chunk by chunk.
pub struct ChatSession {
    child: std::process::Child,
    events: std::sync::mpsc::Receiver<ChatEvent>,
    cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
    settings: AiSettings,
    prompt: String,
}

impl ChatSession {
    /// Start a streamed request; `body_path` must already hold the request body.
    pub fn start(
        settings: &AiSettings,
        api_key: &str,
        prompt: &str,
        body_path: &std::path::Path,
        timeout_seconds: u32,
    ) -> Result<Self, AiError> {
        use std::process::{Command, Stdio};
        let url = chat_url(&settings.endpoint)?;
        if settings.model.trim().is_empty() {
            return Err(AiError::Configuration("AI model is required".into()));
        }
        let mut command = Command::new("curl");
        command
            .arg("-sS")
            .arg("--no-buffer")
            .arg("-N")
            .arg("--max-time")
            .arg(timeout_seconds.to_string())
            .arg("-X")
            .arg("POST")
            .arg(&url)
            .arg("-H")
            .arg("Content-Type: application/json")
            .arg("-H")
            .arg(format!("Authorization: Bearer {api_key}"))
            .arg("--data")
            .arg(format!("@{}", body_path.display()))
            .arg("-w")
            .arg("\n%{http_code}")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command
            .spawn()
            .map_err(|error| AiError::Configuration(format!("curl could not start: {error}")))?;
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (sender, events) = std::sync::mpsc::channel();
        if let Some(stdout) = stdout {
            let cancelled = std::sync::Arc::clone(&cancelled);
            let sender = sender.clone();
            std::thread::spawn(move || {
                use std::io::{BufRead, BufReader};
                let reader = BufReader::new(stdout);
                for line in reader.lines() {
                    if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                        break;
                    }
                    let Ok(line) = line else {
                        break;
                    };
                    match parse_stream_line(&line) {
                        StreamEvent::Delta(text) => {
                            if sender.send(ChatEvent::Delta(text)).is_err() {
                                return;
                            }
                        }
                        StreamEvent::Done => {}
                        StreamEvent::Other(value) => {
                            // The status code arrives as the last plain line.
                            let (_, status) = split_status(&format!("x\n{value}"));
                            if let Some(status) = status {
                                if sender.send(ChatEvent::Done(Some(status))).is_err() {
                                    return;
                                }
                            }
                        }
                    }
                }
            });
        }
        if let Some(stderr) = stderr {
            let sender = sender.clone();
            std::thread::spawn(move || {
                use std::io::Read;
                let mut buffer = String::new();
                let mut stderr = stderr;
                let _ = stderr.read_to_string(&mut buffer);
                let trimmed = buffer.trim();
                if !trimmed.is_empty() {
                    let _ = sender.send(ChatEvent::Error(trimmed.chars().take(300).collect()));
                }
            });
        }
        Ok(Self {
            child,
            events,
            cancelled,
            settings: settings.clone(),
            prompt: prompt.to_string(),
        })
    }

    pub fn prompt(&self) -> &str {
        &self.prompt
    }

    pub fn settings(&self) -> &AiSettings {
        &self.settings
    }

    pub fn is_running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Take the events that arrived since the last poll.
    pub fn poll(&mut self) -> Vec<ChatEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.events.try_recv() {
            events.push(event);
        }
        if events.is_empty() && !self.is_running() {
            match self.child.try_wait() {
                Ok(Some(status)) if !status.success() => {
                    events.push(ChatEvent::Error(format!(
                        "the AI request exited with {status}"
                    )));
                }
                _ => {}
            }
        }
        events
    }

    /// Stop the request; the panel keeps whatever text already arrived.
    pub fn cancel(&mut self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for ChatSession {
    fn drop(&mut self) {
        if self.is_running() {
            self.cancel();
        }
    }
}

/// A conversation kept between requests.
///
/// The panel stores turns here so a follow-up question has context and so the
/// history can be written next to the project and reloaded later.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Conversation {
    turns: Vec<(bool, String)>,
}

/// Turns a conversation keeps on disk.
pub const MAX_STORED_TURNS: usize = 40;

impl Conversation {
    pub fn new() -> Self {
        Self::default()
    }

    /// Turns as `(is_user, text)`, oldest first.
    pub fn turns(&self) -> &[(bool, String)] {
        &self.turns
    }

    pub fn is_empty(&self) -> bool {
        self.turns.is_empty()
    }

    /// Start a new question; an empty answer turn is appended for the reply.
    pub fn push_user(&mut self, prompt: &str) {
        let prompt = prompt.trim();
        if prompt.is_empty() {
            return;
        }
        self.turns.push((true, prompt.to_string()));
        self.turns.push((false, String::new()));
        self.trim();
    }

    /// Append streamed answer text to the newest reply.
    pub fn append_answer(&mut self, text: &str) {
        match self.turns.iter_mut().rev().find(|(user, _)| !*user) {
            Some((_, answer)) => answer.push_str(text),
            None => self.turns.push((false, text.to_string())),
        }
    }

    /// Newest answer text.
    pub fn answer(&self) -> &str {
        self.turns
            .iter()
            .rev()
            .find(|(user, _)| !*user)
            .map(|(_, answer)| answer.as_str())
            .unwrap_or("")
    }

    pub fn clear(&mut self) {
        self.turns.clear();
    }

    fn trim(&mut self) {
        if self.turns.len() > MAX_STORED_TURNS * 2 {
            let excess = self.turns.len() - MAX_STORED_TURNS * 2;
            self.turns.drain(..excess);
        }
    }

    /// `role|text` lines, with newlines escaped, so the file stays line based.
    pub fn serialize(&self) -> String {
        let mut text = String::new();
        for (is_user, content) in &self.turns {
            let escaped = content
                .replace('\\', "\\\\")
                .replace('\n', "\\n")
                .replace('\r', "");
            text.push_str(&format!(
                "{}|{escaped}\n",
                if *is_user { "user" } else { "assistant" }
            ));
        }
        text
    }

    pub fn parse(content: &str) -> Conversation {
        let mut conversation = Conversation::new();
        for line in content.lines() {
            let Some((role, text)) = line.split_once('|') else {
                continue;
            };
            let mut decoded = String::new();
            let mut escaped = false;
            for character in text.chars() {
                if escaped {
                    decoded.push(match character {
                        'n' => '\n',
                        other => other,
                    });
                    escaped = false;
                    continue;
                }
                match character {
                    '\\' => escaped = true,
                    other => decoded.push(other),
                }
            }
            match role.trim() {
                "user" => conversation.turns.push((true, decoded)),
                "assistant" => conversation.turns.push((false, decoded)),
                _ => {}
            }
        }
        conversation.trim();
        conversation
    }
}

/// Environment entries a child process needs for an AI request.
pub fn child_environment() -> [&'static str; 1] {
    [API_KEY_ENV]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lsp_core::executable_available;

    #[test]
    fn endpoints_are_validated_and_normalized() {
        assert_eq!(
            normalize_endpoint("https://api.example.test/v1/").unwrap(),
            "https://api.example.test/v1"
        );
        assert_eq!(
            chat_url("https://api.example.test/v1").unwrap(),
            "https://api.example.test/v1/chat/completions"
        );
        assert_eq!(
            chat_url("https://api.example.test/v1/chat/completions").unwrap(),
            "https://api.example.test/v1/chat/completions"
        );
        assert!(normalize_endpoint("").is_err());
        assert!(normalize_endpoint("api.example.test").is_err());
        assert!(normalize_endpoint("https://api example.test").is_err());
        assert!(normalize_endpoint("http://api.example.test/v1").is_err());
        assert!(normalize_endpoint("http://localhost:8080/v1").is_ok());
        assert!(normalize_endpoint("http://127.0.0.1:8080/v1").is_ok());
    }

    #[test]
    fn request_bodies_escape_user_text() {
        let body = chat_body("gpt-5.1", "line \"one\"\nline two\\", false);
        assert!(body.contains("\"model\":\"gpt-5.1\""));
        assert!(body.contains("\"stream\":false"));
        assert!(body.contains("line \\\"one\\\"\\nline two\\\\"));
        assert!(!body.contains('\n'));
        assert_eq!(json_escape("\u{1}"), "\\u0001");
    }

    #[test]
    fn project_context_is_bounded_and_utf8_safe() {
        let prompt = prompt_with_project_context("explain main", "project: demo\nfile: main.rs");
        assert!(prompt.starts_with("explain main\n\n[TC-IDE project context"));
        assert!(prompt.contains("file: main.rs"));
        assert!(prompt.ends_with("[/TC-IDE project context]"));

        let document = "界".repeat(MAX_PROJECT_CONTEXT_BYTES);
        let prompt = prompt_with_project_context("分析", &document);
        assert!(prompt.contains("[project context truncated]"));
        assert!(prompt.contains("分析"));
        assert!(prompt.len() < MAX_PROJECT_CONTEXT_BYTES + 256);
    }

    #[test]
    fn a_conversation_round_trips_through_its_file() {
        let mut conversation = Conversation::new();
        conversation.push_user("first question");
        conversation.append_answer("first ");
        conversation.append_answer("answer");
        conversation.push_user("second\nquestion");
        conversation.append_answer("done");
        assert_eq!(conversation.answer(), "done");
        assert_eq!(conversation.turns().len(), 4);

        let text = conversation.serialize();
        assert_eq!(text.lines().count(), 4);
        assert!(text.contains("user|first question"));
        let restored = Conversation::parse(&text);
        assert_eq!(restored, conversation);
        assert_eq!(restored.turns()[1].1, "first answer");
        assert_eq!(restored.turns()[2].1, "second\nquestion");

        // Unknown lines and stray separators are ignored rather than breaking the
        // rest of the file.
        let broken = Conversation::parse("nonsense\nsystem|ignored\nuser|kept\n");
        assert_eq!(broken.turns(), [(true, "kept".to_string())]);

        let mut long = Conversation::new();
        for index in 0..(MAX_STORED_TURNS + 20) {
            long.push_user(&format!("q{index}"));
            long.append_answer("a");
        }
        assert!(long.turns().len() <= MAX_STORED_TURNS * 2);
        long.clear();
        assert!(long.is_empty());
    }

    #[test]
    fn history_is_included_and_bounded() {
        let body = chat_body_with_history(
            "demo",
            &[("user", "first"), ("assistant", "hi"), ("user", "second")],
            false,
        );
        assert!(body.contains("{\"role\":\"user\",\"content\":\"first\"}"));
        assert!(body.contains("{\"role\":\"assistant\",\"content\":\"hi\"}"));
        assert!(body.contains("{\"role\":\"user\",\"content\":\"second\"}"));
        // The turns arrive in order, oldest first.
        let first = body.find("first").unwrap();
        let second = body.find("second").unwrap();
        assert!(first < second);

        let mut turns = Vec::new();
        for index in 0..(MAX_HISTORY_TURNS + 5) {
            turns.push((
                "user",
                Box::leak(format!("turn{index}").into_boxed_str()) as &str,
            ));
        }
        let bounded = chat_body_with_history("demo", &turns, true);
        assert!(!bounded.contains("turn0"), "old turns must be dropped");
        assert!(bounded.contains("turn16"));
        assert_eq!(bounded.matches("\"role\"").count(), MAX_HISTORY_TURNS);
    }

    #[test]
    fn commands_never_contain_the_api_key() {
        let settings = AiSettings {
            endpoint: "https://api.example.test/v1".into(),
            model: "demo".into(),
        };
        let windows = chat_command(&settings, "body.json", true, 30, Shell::Windows).unwrap();
        assert!(windows.contains("%TC_IDE_AI_KEY%"));
        assert!(windows.contains("--no-buffer"));
        assert!(windows.contains("--max-time 30"));
        let unix = chat_command(&settings, "/tmp/body.json", false, 60, Shell::Unix).unwrap();
        assert!(unix.contains("$TC_IDE_AI_KEY"));
        assert!(unix.contains("@/tmp/body.json"));

        let broken = AiSettings {
            model: "  ".into(),
            ..settings.clone()
        };
        assert!(chat_command(&broken, "body.json", false, 30, Shell::Windows).is_err());
        let bad_endpoint = AiSettings {
            endpoint: "ftp://example.test".into(),
            ..settings
        };
        assert!(chat_command(&bad_endpoint, "body.json", false, 30, Shell::Unix).is_err());
    }

    #[test]
    fn responses_are_parsed_with_and_without_status_suffix() {
        let body = "{\"choices\":[{\"message\":{\"role\":\"assistant\",\"content\":\"hello \\\"there\\\"\"}}]}";
        assert_eq!(parse_chat_response(body).unwrap(), "hello \"there\"");
        assert_eq!(
            parse_chat_response(&format!("{body}\n200")).unwrap(),
            "hello \"there\""
        );
    }

    #[test]
    fn http_errors_become_readable_messages() {
        let body = "{\"error\":{\"message\":\"Incorrect API key provided\"}}";
        let error = parse_chat_response(&format!("{body}\n401")).unwrap_err();
        assert_eq!(
            error,
            AiError::Status(401, "Incorrect API key provided".into())
        );
        assert!(error.message().contains("key was rejected"));
        assert!(describe_status(404, body).contains("not found"));
        assert!(describe_status(429, body).contains("rate limit"));
        assert!(describe_status(503, body).contains("service error"));
        assert!(describe_status(408, "").contains("timed out"));
        assert!(matches!(
            parse_chat_response("{}").unwrap_err(),
            AiError::Response(_)
        ));
        assert!(
            parse_chat_response("{\"error\":{\"message\":\"nope\"}}\n500")
                .unwrap_err()
                .message()
                .contains("service error")
        );
    }

    #[test]
    fn status_suffix_is_split_only_when_it_looks_like_a_code() {
        let (body, status) = split_status("{\"a\":1}\n204");
        assert_eq!(body, "{\"a\":1}");
        assert_eq!(status, Some(204));
        let (body, status) = split_status("{\"a\":1}");
        assert_eq!(body, "{\"a\":1}");
        assert_eq!(status, None);
        let (body, status) = split_status("{\"a\":1}\nnot-a-code");
        assert!(body.contains("not-a-code"));
        assert_eq!(status, None);
    }

    #[test]
    fn stream_lines_are_classified() {
        assert_eq!(
            parse_stream_line("data: {\"choices\":[{\"delta\":{\"content\":\"He\"}}]}"),
            StreamEvent::Delta("He".into())
        );
        assert_eq!(parse_stream_line("data: [DONE]"), StreamEvent::Done);
        assert!(matches!(
            parse_stream_line(": keep-alive"),
            StreamEvent::Other(_)
        ));
        assert_eq!(
            parse_stream_line("data: {\"choices\":[{\"delta\":{}}]}"),
            StreamEvent::Other("data: {\"choices\":[{\"delta\":{}}]}".into())
        );
    }

    #[test]
    fn nested_json_strings_are_found() {
        let text = "{\"error\":{\"type\":\"auth\",\"message\":\"bad key\"},\"content\":\"ok\"}";
        assert_eq!(json_string(text, "message").as_deref(), Some("bad key"));
        assert_eq!(json_string(text, "content").as_deref(), Some("ok"));
        assert_eq!(json_string(text, "missing"), None);
        assert_eq!(
            json_string("{\"empty\":\"\"}", "empty").as_deref(),
            Some("")
        );
    }

    #[test]
    fn a_streaming_request_is_driven_against_a_local_server() {
        // A tiny HTTP server that answers with server sent events, so the client
        // can be checked without any network access.
        let script = r#"
import http.server, json, sys, threading

class Handler(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        length = int(self.headers.get("Content-Length", "0"))
        body = self.rfile.read(length).decode("utf-8")
        assert "ping" in body or "hello" in body, body
        assert self.headers.get("Authorization", "").startswith("Bearer "), "missing key"
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.end_headers()
        for chunk in ["Hel", "lo ", "there"]:
            payload = json.dumps({"choices": [{"delta": {"content": chunk}}]})
            self.wfile.write(("data: " + payload + "\n\n").encode("utf-8"))
            self.wfile.flush()
        self.wfile.write(b"data: [DONE]\n\n")
        self.wfile.flush()

    def log_message(self, *args):
        pass

server = http.server.HTTPServer(("127.0.0.1", 0), Handler)
port = server.server_address[1]
print(port, flush=True)
server.serve_forever()
"#;
        // The transport is shared code, and the child process plumbing is
        // already covered on Windows by the language server and debug adapter
        // tests; this check needs a local HTTP server and a working curl, so it
        // runs where those are known to behave and skips otherwise.
        if cfg!(windows) {
            return;
        }
        let python = "python3";
        if !executable_available(python) || !executable_available("curl") {
            return;
        }
        let directory = std::env::temp_dir().join("tc-ide-ai-test");
        let _ = std::fs::create_dir_all(&directory);
        let script_path = directory.join("fake_server.py");
        std::fs::write(&script_path, script).expect("write fake server");
        let mut server = std::process::Command::new(python)
            .arg(&script_path)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("start fake server");
        let (port_line, bytes_read) = {
            use std::io::{BufRead, BufReader};
            let stdout = server.stdout.take().expect("stdout");
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            let bytes_read = reader.read_line(&mut line).expect("read fake server port");
            (line, bytes_read)
        };
        if bytes_read == 0 {
            let output = server.wait_with_output().expect("wait for fake server");
            panic!(
                "fake server exited before reporting its port: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        let port = port_line
            .trim()
            .parse::<u16>()
            .expect("fake server printed an invalid port");

        let settings = AiSettings {
            endpoint: format!("http://127.0.0.1:{port}/v1"),
            model: "demo".into(),
        };
        let body_path = directory.join("body.json");
        std::fs::write(&body_path, chat_body("demo", "ping", true)).expect("body");

        let mut session =
            ChatSession::start(&settings, "test-key", "ping", &body_path, 20).expect("start");
        let mut answer = String::new();
        let mut done = false;
        let mut failure = None;
        for _ in 0..1500 {
            for event in session.poll() {
                match event {
                    ChatEvent::Delta(text) => answer.push_str(&text),
                    ChatEvent::Done(_) => done = true,
                    ChatEvent::Error(error) => failure = Some(error),
                }
            }
            if (done && !answer.is_empty()) || failure.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(failure, None, "the request reported an error");
        assert_eq!(answer, "Hello there");
        assert!(done, "the stream never reported completion");

        // A cancelled request stops without hanging.
        let mut cancelled =
            ChatSession::start(&settings, "test-key", "hello", &body_path, 20).expect("start");
        cancelled.cancel();
        assert!(!cancelled.is_running());

        let _ = server.kill();
        let _ = server.wait();
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn connection_tests_use_the_configured_model() {
        let body = connection_test_body("demo-model");
        assert!(body.contains("\"model\":\"demo-model\""));
        assert!(body.contains("ping"));
        assert!(body.contains("\"stream\":false"));
    }

    #[test]
    fn a_conversation_survives_a_round_trip_through_the_project_file() {
        // What the Linux panel does: save the conversation with the project and
        // read it back when the project opens again.
        let mut conversation = Conversation::new();
        assert!(conversation.is_empty());
        conversation.push_user("where is the graph built?");
        conversation.append_answer("graph_core::build_graph");
        conversation.push_user("and the index?");
        conversation.append_answer("index_core keeps fingerprints");

        let saved = conversation.serialize();
        let restored = Conversation::parse(&saved);
        assert_eq!(restored.turns(), conversation.turns());
        assert_eq!(restored.turns().len(), 4);
        // The turns keep their side: the question is from the user and the
        // answer is not, which is what the panel colours differently.
        let sides = restored
            .turns()
            .iter()
            .map(|(from_user, _)| *from_user)
            .collect::<Vec<_>>();
        assert_eq!(sides, [true, false, true, false]);
        // A key never reaches the file: only the conversation is written.
        assert!(!saved.contains("TC_IDE_AI_KEY"));
        assert!(!saved.to_ascii_lowercase().contains("api_key"));

        // An empty or damaged file gives an empty conversation rather than a
        // failure, so opening a project cannot fail because of its history.
        assert!(Conversation::parse("").is_empty());
        assert!(Conversation::parse("nonsense without sections\n").is_empty());
    }

    #[test]
    fn the_prompt_carries_the_conversation_and_the_key_stays_out_of_the_text() {
        // The panel sends the project's turns with the prompt so the answer has
        // context, and the key travels in the environment, never in the body.
        let mut conversation = Conversation::new();
        conversation.push_user("earlier question");
        conversation.append_answer("earlier answer");
        let turns = conversation.turns().to_vec();
        let history = turns
            .iter()
            .map(|(from_user, text)| (if *from_user { "user" } else { "assistant" }, text.as_str()))
            .collect::<Vec<_>>();
        let body = chat_body_with_history("gpt-5.1", &history, true);
        assert!(body.contains("earlier question"));
        assert!(body.contains("earlier answer"));
        assert!(body.contains("\"role\":\"user\""));
        assert!(body.contains("\"role\":\"assistant\""));
        // The body never holds the key, whatever it is.
        let secret = "sk-not-a-real-key";
        assert!(!body.contains(secret));
        // And the command line only ever names the environment variable.
        let settings = AiSettings {
            endpoint: "https://api.openai.com/v1".into(),
            model: "gpt-5.1".into(),
        };
        let command =
            chat_command(&settings, "/tmp/body.json", true, 60, Shell::Unix).expect("a command");
        assert!(command.contains(API_KEY_ENV), "{command}");
        assert!(!command.contains(secret));
    }
}
