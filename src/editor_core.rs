#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind {
    Plain,
    Keyword,
    Literal,
    Comment,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyntaxToken {
    pub text: String,
    pub kind: TokenKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymbolRole {
    Declaration,
    Reference,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymbolOccurrence {
    pub name: String,
    pub line: usize,
    pub start: usize,
    pub end: usize,
    pub role: SymbolRole,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectSymbolOccurrence {
    pub file: String,
    pub symbol: SymbolOccurrence,
}

fn identifier_ranges(line: &str) -> Vec<(usize, usize, String)> {
    let chars = line.chars().collect::<Vec<_>>();
    let mut result = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        if !chars[index].is_ascii_alphabetic() && chars[index] != '_' {
            index += 1;
            continue;
        }
        let start = index;
        index += 1;
        while index < chars.len() && (chars[index].is_ascii_alphanumeric() || chars[index] == '_') {
            index += 1;
        }
        result.push((start, index, chars[start..index].iter().collect()));
    }
    result
}

fn declaration_name(line: &str, file: &str) -> Option<String> {
    let code = comment_bounds(line, file)
        .map(|(start, _, _)| &line[..start])
        .unwrap_or(line);
    let identifiers = identifier_ranges(code);
    for pair in identifiers.windows(2) {
        if matches!(
            pair[0].2.as_str(),
            "fn" | "def" | "func" | "function" | "class" | "struct" | "enum" | "trait"
        ) {
            return Some(pair[1].2.clone());
        }
    }
    let open = code.find('(')?;
    let before = &code[..open];
    let candidate = identifier_ranges(before).pop()?;
    if matches!(
        candidate.2.as_str(),
        "if" | "for" | "while" | "switch" | "catch"
    ) {
        return None;
    }
    let prefix = before[..before
        .char_indices()
        .nth(candidate.0)
        .map(|(index, _)| index)
        .unwrap_or(before.len())]
        .trim();
    if prefix.is_empty() || prefix.ends_with('.') || prefix.ends_with("->") {
        None
    } else {
        Some(candidate.2)
    }
}

pub fn symbol_occurrences(lines: &[String], file: &str) -> Vec<SymbolOccurrence> {
    let declarations = lines
        .iter()
        .filter_map(|line| declaration_name(line, file))
        .collect::<Vec<_>>();
    let mut result = Vec::new();
    for (line_index, line) in lines.iter().enumerate() {
        let code = comment_bounds(line, file)
            .map(|(start, _, _)| &line[..start])
            .unwrap_or(line);
        let declared = declaration_name(line, file);
        for (start, end, name) in identifier_ranges(code) {
            if declarations.contains(&name) {
                result.push(SymbolOccurrence {
                    role: if declared.as_deref() == Some(name.as_str()) {
                        SymbolRole::Declaration
                    } else {
                        SymbolRole::Reference
                    },
                    name,
                    line: line_index,
                    start,
                    end,
                });
            }
        }
    }
    result
}

pub fn project_symbol_occurrences(
    sources: &[(String, Vec<String>)],
) -> Vec<ProjectSymbolOccurrence> {
    let declarations = sources
        .iter()
        .flat_map(|(file, lines)| lines.iter().filter_map(|line| declaration_name(line, file)))
        .collect::<Vec<_>>();
    let mut result = Vec::new();
    for (file, lines) in sources {
        for (line_index, line) in lines.iter().enumerate() {
            let code = comment_bounds(line, file)
                .map(|(start, _, _)| &line[..start])
                .unwrap_or(line);
            let declared = declaration_name(line, file);
            for (start, end, name) in identifier_ranges(code) {
                if declarations.contains(&name) {
                    result.push(ProjectSymbolOccurrence {
                        file: file.clone(),
                        symbol: SymbolOccurrence {
                            role: if declared.as_deref() == Some(name.as_str()) {
                                SymbolRole::Declaration
                            } else {
                                SymbolRole::Reference
                            },
                            name,
                            line: line_index,
                            start,
                            end,
                        },
                    });
                }
            }
        }
    }
    result
}

pub fn symbol_at(
    lines: &[String],
    file: &str,
    line: usize,
    column: usize,
) -> Option<SymbolOccurrence> {
    symbol_occurrences(lines, file)
        .into_iter()
        .find(|symbol| symbol.line == line && column >= symbol.start && column <= symbol.end)
}

pub fn language_for_file(name: &str) -> &'static str {
    match name
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "c" | "h" => "C",
        "cc" | "cpp" | "cxx" | "hpp" | "hh" => "C++",
        "rs" => "Rust",
        "py" | "pyw" => "Python",
        "java" => "Java",
        "kt" | "kts" => "Kotlin",
        "js" | "jsx" | "mjs" | "ts" | "tsx" => "TypeScript / JavaScript",
        "go" => "Go",
        "html" | "htm" | "css" | "scss" | "sass" | "vue" | "svelte" => "Web / HTML / CSS",
        "cs" => "C#",
        "swift" => "Swift",
        "zig" => "Zig",
        "lua" => "Lua",
        "rb" => "Ruby",
        "php" => "PHP",
        "r" => "R",
        "sql" => "SQL",
        "sh" | "bash" | "zsh" | "fish" => "Shell",
        "json" | "jsonc" => "JSON",
        "toml" => "TOML",
        "yaml" | "yml" => "YAML",
        "md" | "mdx" => "Markdown",
        _ => "Plain Text",
    }
}

pub fn comment_markers(name: &str) -> &'static [&'static str] {
    match name
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "py" | "pyw" | "rb" | "sh" | "bash" | "zsh" | "fish" | "r" => &["#"],
        "sql" | "lua" | "hs" | "lhs" => &["--"],
        "html" | "htm" | "md" | "mdx" => &["<!--"],
        "css" | "scss" | "sass" => &["/*"],
        "c" | "h" | "cc" | "cpp" | "cxx" | "hpp" | "rs" | "java" | "kt" | "kts" | "swift"
        | "go" | "cs" | "ts" | "tsx" | "js" | "jsx" | "mjs" | "dart" | "php" | "scala" | "vue"
        | "svelte" => &["//", "/*"],
        _ => &[],
    }
}

pub fn comment_bounds(line: &str, file: &str) -> Option<(usize, usize, usize)> {
    let (marker_start, marker) = comment_markers(file)
        .iter()
        .filter_map(|marker| line.find(marker).map(|index| (index, *marker)))
        .min_by_key(|(index, _)| *index)?;
    if marker == "#" && marker_start == 0 && line.starts_with("#!") {
        return None;
    }
    let marker_end = marker_start + marker.len();
    let after = &line[marker_end..];
    let leading = after.len() - after.trim_start().len();
    let content_start = marker_end + leading;
    let closing = match marker {
        "/*" => "*/",
        "<!--" => "-->",
        _ => "",
    };
    let raw_end = if closing.is_empty() {
        line.len()
    } else {
        after
            .find(closing)
            .map(|offset| marker_end + offset)
            .unwrap_or(line.len())
    };
    let content_end = content_start + line[content_start..raw_end].trim_end().len();
    Some((marker_start, content_start, content_end))
}

pub fn completion_words(language: &str) -> &'static [&'static str] {
    match language {
        "C" => &[
            "auto", "break", "case", "char", "const", "continue", "define", "double", "else",
            "enum", "extern", "float", "for", "if", "include", "inline", "int", "long", "NULL",
            "return", "short", "signed", "sizeof", "size_t", "static", "struct", "switch",
            "typedef", "union", "unsigned", "void", "volatile", "while",
        ],
        "C++" => &[
            "alignas",
            "auto",
            "bool",
            "break",
            "case",
            "catch",
            "char",
            "class",
            "concept",
            "const",
            "constexpr",
            "delete",
            "else",
            "enum",
            "explicit",
            "export",
            "false",
            "float",
            "for",
            "friend",
            "if",
            "include",
            "inline",
            "int",
            "namespace",
            "new",
            "nullptr",
            "operator",
            "private",
            "protected",
            "public",
            "requires",
            "return",
            "static",
            "std",
            "string",
            "struct",
            "template",
            "this",
            "throw",
            "true",
            "try",
            "typename",
            "using",
            "vector",
            "virtual",
            "void",
            "while",
        ],
        "Rust" => &[
            "async", "await", "const", "crate", "enum", "fn", "impl", "let", "match", "mod", "mut",
            "pub", "Result", "self", "struct", "trait", "use", "where",
        ],
        "Python" => &[
            "and", "async", "await", "class", "def", "elif", "else", "except", "False", "for",
            "from", "if", "import", "in", "lambda", "None", "not", "or", "return", "self", "True",
            "try", "with", "yield",
        ],
        "TypeScript / JavaScript" | "Web / HTML / CSS" => &[
            "async",
            "await",
            "class",
            "const",
            "export",
            "extends",
            "function",
            "import",
            "interface",
            "let",
            "new",
            "return",
            "type",
            "var",
        ],
        "Go" => &[
            "chan",
            "const",
            "defer",
            "func",
            "go",
            "import",
            "interface",
            "map",
            "package",
            "range",
            "return",
            "struct",
            "type",
            "var",
        ],
        "Java" | "Kotlin" => &[
            "class",
            "extends",
            "fun",
            "import",
            "interface",
            "new",
            "package",
            "private",
            "protected",
            "public",
            "return",
            "static",
            "val",
            "var",
        ],
        _ => &[
            "class", "const", "function", "import", "return", "struct", "type", "value",
        ],
    }
}

pub fn completion_prefix(line: &str, character_column: usize) -> String {
    line.chars()
        .take(character_column)
        .collect::<String>()
        .chars()
        .rev()
        .take_while(|value| value.is_ascii_alphanumeric() || *value == '_')
        .collect::<String>()
        .chars()
        .rev()
        .collect()
}

pub fn completions(file: &str, line: &str, column: usize) -> Vec<String> {
    let prefix = completion_prefix(line, column);
    if prefix.is_empty() {
        return Vec::new();
    }
    completion_words(language_for_file(file))
        .iter()
        .filter(|word| word.starts_with(&prefix) && **word != prefix)
        .take(6)
        .map(|word| (*word).to_string())
        .collect()
}

pub fn syntax_tokens(value: &str, file: &str) -> Vec<SyntaxToken> {
    if let Some((comment_start, _, _)) = comment_bounds(value, file) {
        let mut tokens = syntax_tokens_without_comment(&value[..comment_start], file);
        tokens.push(SyntaxToken {
            text: value[comment_start..].into(),
            kind: TokenKind::Comment,
        });
        return tokens;
    }
    syntax_tokens_without_comment(value, file)
}

fn syntax_tokens_without_comment(value: &str, file: &str) -> Vec<SyntaxToken> {
    let keywords = completion_words(language_for_file(file));
    let chars = value.chars().collect::<Vec<_>>();
    let mut result = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        let start = index;
        let kind = if chars[index] == '"' || chars[index] == '\'' {
            let quote = chars[index];
            index += 1;
            while index < chars.len() {
                let done = chars[index] == quote && chars[index.saturating_sub(1)] != '\\';
                index += 1;
                if done {
                    break;
                }
            }
            TokenKind::Literal
        } else if chars[index].is_ascii_alphabetic() || chars[index] == '_' {
            index += 1;
            while index < chars.len()
                && (chars[index].is_ascii_alphanumeric() || chars[index] == '_')
            {
                index += 1;
            }
            let word = chars[start..index].iter().collect::<String>();
            result.push(SyntaxToken {
                kind: if keywords.contains(&word.as_str()) {
                    TokenKind::Keyword
                } else {
                    TokenKind::Plain
                },
                text: word,
            });
            continue;
        } else if chars[index].is_ascii_digit() {
            index += 1;
            while index < chars.len()
                && (chars[index].is_ascii_alphanumeric() || chars[index] == '.')
            {
                index += 1;
            }
            TokenKind::Literal
        } else {
            index += 1;
            while index < chars.len()
                && !chars[index].is_ascii_alphanumeric()
                && chars[index] != '_'
                && chars[index] != '"'
                && chars[index] != '\''
            {
                index += 1;
            }
            TokenKind::Plain
        };
        result.push(SyntaxToken {
            text: chars[start..index].iter().collect(),
            kind,
        });
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c_preprocessor_is_not_a_comment() {
        assert_eq!(comment_bounds("#include <stdio.h>", "main.c"), None);
        assert_eq!(language_for_file("main.cpp"), "C++");
    }

    #[test]
    fn completion_follows_the_file_language() {
        assert!(completions("main.cpp", "cons", 4).contains(&"const".into()));
        assert!(!completions("main.c", "cla", 3).contains(&"class".into()));
    }

    #[test]
    fn common_extension_languages_are_detected() {
        assert_eq!(language_for_file("main.zig"), "Zig");
        assert_eq!(language_for_file("plugin.lua"), "Lua");
        assert_eq!(language_for_file("task.rb"), "Ruby");
        assert_eq!(language_for_file("index.php"), "PHP");
    }

    #[test]
    fn comments_are_separate_syntax_tokens() {
        let tokens = syntax_tokens("let value = 3; // note", "main.rs");
        assert_eq!(
            tokens.last().map(|token| token.kind),
            Some(TokenKind::Comment)
        );
    }

    #[test]
    fn semantic_symbols_connect_declarations_and_references() {
        let lines = vec![
            "fn render() {".to_string(),
            "    render();".to_string(),
            "}".to_string(),
        ];
        let symbols = symbol_occurrences(&lines, "main.rs");
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].role, SymbolRole::Declaration);
        assert_eq!(symbols[1].role, SymbolRole::Reference);
        assert_eq!(symbol_at(&lines, "main.rs", 1, 7).unwrap().name, "render");
    }

    #[test]
    fn project_symbols_connect_calls_across_files() {
        let sources = vec![
            ("src/api.rs".into(), vec!["pub fn launch() {}".into()]),
            ("src/main.rs".into(), vec!["launch();".into()]),
        ];
        let symbols = project_symbol_occurrences(&sources);
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].symbol.role, SymbolRole::Declaration);
        assert_eq!(symbols[1].file, "src/main.rs");
        assert_eq!(symbols[1].symbol.role, SymbolRole::Reference);
    }
}
