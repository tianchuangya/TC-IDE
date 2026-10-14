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
    format!(
        "{{\"model\":\"{}\",\"stream\":{},\"messages\":[{{\"role\":\"user\",\"content\":\"{}\"}}]}}",
        json_escape(model),
        if stream { "true" } else { "false" },
        json_escape(prompt)
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

/// Environment entries a child process needs for an AI request.
pub fn child_environment() -> [&'static str; 1] {
    [API_KEY_ENV]
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn connection_tests_use_the_configured_model() {
        let body = connection_test_body("demo-model");
        assert!(body.contains("\"model\":\"demo-model\""));
        assert!(body.contains("ping"));
        assert!(body.contains("\"stream\":false"));
    }
}
