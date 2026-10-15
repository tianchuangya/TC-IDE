/// Syntax token kinds. The Windows backend draws them; the Linux backend has its
/// own tokenizer, so the shared version is allowed to be unused on Linux.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind {
    Plain,
    Keyword,
    Type,
    Function,
    /// A local binding introduced by `let`, `var`, `val` and friends.
    Variable,
    /// A name declared in a parameter list.
    Parameter,
    /// A name reached through `.` or `::`, or a struct field.
    Property,
    /// A macro or an annotation, such as `println!` or `@Override`.
    Macro,
    /// A module, namespace or crate name.
    Module,
    Constant,
    Preprocessor,
    Literal,
    Comment,
}

#[allow(dead_code)]
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

pub fn identifier_ranges(line: &str) -> Vec<(usize, usize, String)> {
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

const FLOW_KEYWORDS: [&str; 16] = [
    "if", "else", "elif", "for", "while", "switch", "case", "catch", "match", "return", "await",
    "try", "loop", "throw", "unless", "yield",
];

const DECLARATION_MODIFIERS: [&str; 14] = [
    "pub",
    "public",
    "private",
    "protected",
    "internal",
    "export",
    "declare",
    "static",
    "readonly",
    "final",
    "open",
    "unsafe",
    "async",
    "extern",
];

/// Name of a `let`/`const`/`var` style declaration, if the line starts one.
fn variable_declaration(code: &str, identifiers: &[(usize, usize, String)]) -> Option<String> {
    for (index, (_, _, word)) in identifiers.iter().enumerate() {
        let strict = match word.as_str() {
            "let" | "const" | "var" | "val" => false,
            // `static` is also a Java/C# modifier, so it only declares a name when
            // a type annotation or initialiser follows directly.
            "static" => true,
            _ => continue,
        };
        let modifiers_only = identifiers[..index].iter().all(|(_, _, earlier)| {
            earlier != "static" && DECLARATION_MODIFIERS.contains(&earlier.as_str())
        });
        if !modifiers_only {
            continue;
        }
        let mut next = index + 1;
        while matches!(
            identifiers.get(next).map(|(_, _, name)| name.as_str()),
            Some("mut" | "ref" | "final")
        ) {
            next += 1;
        }
        let Some((_, end, name)) = identifiers.get(next) else {
            continue;
        };
        if strict {
            let after = code
                .char_indices()
                .nth(*end)
                .map(|(offset, _)| &code[offset..])
                .unwrap_or("")
                .trim_start();
            if !after.starts_with(':') && !after.starts_with('=') {
                continue;
            }
        }
        return Some(name.clone());
    }
    None
}

/// `name(args) {` or `name(args):` defines a method in languages that have no
/// declaration keyword, such as Java, C# and TypeScript. Requiring the
/// definition shape keeps plain calls (`helper();`) out.
fn bare_definition(code: &str) -> Option<String> {
    let trimmed = code.trim_end();
    if !(trimmed.ends_with('{') || trimmed.ends_with(':')) {
        return None;
    }
    let open = code.find('(')?;
    let close = code.rfind(')')?;
    if close < open {
        return None;
    }
    let before = &code[..open];
    if !before.trim().is_empty()
        && !before
            .trim()
            .chars()
            .all(|value| value.is_alphanumeric() || matches!(value, '_' | ' ' | '\t' | '\'' | '#'))
    {
        // Something like `if (x) {` or `value.map(|x| {` is not a definition.
        return None;
    }
    let names = identifier_ranges(before);
    // Exactly one identifier: `start() {`. A prefixed definition such as
    // `public void start() {` is handled by the caller argument heuristic, and
    // `if let Some(x) {` must not look like a definition.
    if names.len() != 1 {
        return None;
    }
    let candidate = names.last()?;
    if matches!(
        candidate.2.as_str(),
        "if" | "for" | "while" | "switch" | "catch" | "match" | "else" | "do" | "return"
    ) {
        return None;
    }
    if before.contains('.') || before.contains("::") || before.contains("->") {
        return None;
    }
    Some(candidate.2.clone())
}

pub fn declaration_name(line: &str, file: &str) -> Option<String> {
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
    if let Some(name) = variable_declaration(code, &identifiers) {
        return Some(name);
    }
    if let Some(name) = bare_definition(code) {
        return Some(name);
    }
    let open = code.find('(')?;
    let before = &code[..open];
    let candidates = identifier_ranges(before);
    let candidate = candidates.last()?.clone();
    if matches!(
        candidate.2.as_str(),
        "if" | "for" | "while" | "switch" | "catch"
    ) {
        return None;
    }
    // `if let Some(value) = ...` and friends are statements, not declarations.
    if candidates[..candidates.len() - 1]
        .iter()
        .any(|(_, _, word)| FLOW_KEYWORDS.contains(&word.as_str()))
    {
        return None;
    }
    let prefix = before[..before
        .char_indices()
        .nth(candidate.0)
        .map(|(index, _)| index)
        .unwrap_or(before.len())]
        .trim();
    // `Engine::run();` is a call, while the out of line definition
    // `void Engine::run() {` is not: only a statement can be a call.
    let statement = code.trim_end().ends_with(';');
    if prefix.is_empty()
        || prefix.ends_with('.')
        || prefix.ends_with("->")
        || (statement && prefix.ends_with("::"))
    {
        None
    } else {
        Some(candidate.2)
    }
}

/// Kind of a declaration, used by the relation graph hierarchy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymbolKind {
    Class,
    Function,
    Variable,
    Other,
}

const CLASS_KEYWORDS: [&str; 13] = [
    "class",
    "struct",
    "interface",
    "enum",
    "trait",
    "impl",
    "record",
    "union",
    "namespace",
    "module",
    "object",
    "type",
    "protocol",
];
const FUNCTION_KEYWORDS: [&str; 9] = [
    "fn",
    "def",
    "func",
    "function",
    "sub",
    "proc",
    "method",
    "constructor",
    "lambda",
];
const VARIABLE_KEYWORDS: [&str; 7] = ["let", "const", "var", "static", "val", "auto", "final"];

/// Classify a declaration line so the graph can nest members under their type.
pub fn declaration_kind(line: &str, file: &str) -> SymbolKind {
    let code = comment_bounds(line, file)
        .map(|(start, _, _)| &line[..start])
        .unwrap_or(line);
    let words = identifier_ranges(code)
        .into_iter()
        .map(|(_, _, name)| name)
        .collect::<Vec<_>>();
    if words
        .iter()
        .any(|word| CLASS_KEYWORDS.contains(&word.as_str()))
    {
        return SymbolKind::Class;
    }
    if words
        .iter()
        .any(|word| FUNCTION_KEYWORDS.contains(&word.as_str()))
    {
        return SymbolKind::Function;
    }
    if words
        .iter()
        .any(|word| VARIABLE_KEYWORDS.contains(&word.as_str()))
        || code.contains('=')
    {
        return SymbolKind::Variable;
    }
    if code.contains('(') {
        return SymbolKind::Function;
    }
    SymbolKind::Other
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

#[allow(dead_code)]
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

#[allow(dead_code)]
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

#[allow(dead_code)]
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

#[allow(dead_code)]
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

#[allow(dead_code)]
/// Names a line introduces as locals, so a later use of the same name can be
/// coloured as a variable even when it is not the declaration itself.
fn declared_names(value: &str) -> Vec<String> {
    let identifiers = identifier_ranges(value);
    let mut names = Vec::new();
    for (index, (_, _, word)) in identifiers.iter().enumerate() {
        if !matches!(word.as_str(), "let" | "var" | "val" | "const" | "auto") {
            continue;
        }
        // The name is the first identifier after the keyword that is not a
        // `mut`-style modifier and not a type.
        for (_, _, candidate) in identifiers.iter().skip(index + 1) {
            if matches!(candidate.as_str(), "mut" | "ref" | "static") {
                continue;
            }
            names.push(candidate.clone());
            break;
        }
    }
    names
}

/// Whether a name sits inside a parameter list of the line.
fn inside_parameters(value: &str, start: usize, end: usize) -> bool {
    // Walk backwards to the nearest unmatched `(` and forwards from there to the
    // matching `)`: a name between them is a parameter.
    let bytes = value.as_bytes();
    let mut depth = 0i32;
    let mut open = None;
    let mut index = start;
    while index > 0 {
        index -= 1;
        match bytes[index] {
            b')' => depth += 1,
            b'(' => {
                if depth == 0 {
                    open = Some(index);
                    break;
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    let Some(open) = open else {
        return false;
    };
    let mut depth = 0i32;
    for (position, byte) in bytes.iter().enumerate().skip(open) {
        match byte {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return end <= position;
                }
            }
            _ => {}
        }
    }
    false
}

/// Whether the character before an identifier is a member accessor.
fn after_member_access(value: &str, start: usize) -> bool {
    let before = value[..start].trim_end();
    before.ends_with('.') || before.ends_with("::") || before.ends_with("->")
}

/// Whether the character after an identifier makes it a macro call.
fn before_macro_bang(value: &str, end: usize) -> bool {
    value[end..].starts_with('!')
}

/// Whether a name is used as a module path word.
fn is_module_word(value: &str, start: usize, end: usize) -> bool {
    let before = value[..start].trim_end();
    let after = value[end..].trim_start();
    // `std::` / `use foo::` style: the word is followed by `::` or preceded by
    // `use`/`mod`/`crate`.
    after.starts_with("::")
        || before.ends_with("use")
        || before.ends_with("mod")
        || before.ends_with("crate")
        || before.ends_with("namespace")
}

fn syntax_tokens_without_comment(value: &str, file: &str) -> Vec<SyntaxToken> {
    let keywords = completion_words(language_for_file(file));
    let chars = value.chars().collect::<Vec<_>>();
    let locals = declared_names(value);
    let mut result = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        let start = index;
        let kind = if chars[index] == '#'
            && value[..value.char_indices().nth(index).map(|v| v.0).unwrap_or(0)]
                .trim()
                .is_empty()
        {
            index += 1;
            while index < chars.len()
                && (chars[index].is_ascii_alphanumeric() || chars[index] == '_')
            {
                index += 1;
            }
            TokenKind::Preprocessor
        } else if chars[index] == '"' || chars[index] == '\'' {
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
            // The helpers read the raw line, so the character range has to be
            // turned back into byte offsets.
            let byte_start = chars[..start].iter().map(|ch| ch.len_utf8()).sum::<usize>();
            let byte_end = byte_start + word.len();
            let next = chars[index..]
                .iter()
                .position(|ch| !ch.is_whitespace())
                .map(|offset| chars[index + offset]);
            let known_type = matches!(
                word.as_str(),
                "bool"
                    | "char"
                    | "double"
                    | "float"
                    | "int"
                    | "long"
                    | "short"
                    | "signed"
                    | "size_t"
                    | "string"
                    | "unsigned"
                    | "void"
                    | "String"
                    | "str"
                    | "usize"
                    | "isize"
                    | "u8"
                    | "u16"
                    | "u32"
                    | "u64"
                    | "u128"
                    | "i8"
                    | "i16"
                    | "i32"
                    | "i64"
                    | "i128"
                    | "f32"
                    | "f64"
            );
            let constant = word.len() > 1
                && word.chars().any(|ch| ch.is_ascii_alphabetic())
                && word
                    .chars()
                    .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_');
            result.push(SyntaxToken {
                kind: if known_type {
                    TokenKind::Type
                } else if constant {
                    TokenKind::Constant
                } else if keywords.contains(&word.as_str()) {
                    TokenKind::Keyword
                } else if word.chars().next().is_some_and(char::is_uppercase) {
                    TokenKind::Type
                } else if next == Some('(') {
                    TokenKind::Function
                } else if before_macro_bang(value, byte_end) {
                    // `println!` and friends read as a macro, not as a bare name.
                    TokenKind::Macro
                } else if after_member_access(value, byte_start) {
                    TokenKind::Property
                } else if inside_parameters(value, byte_start, byte_end) {
                    TokenKind::Parameter
                } else if locals.contains(&word) {
                    TokenKind::Variable
                } else if is_module_word(value, byte_start, byte_end) {
                    TokenKind::Module
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
    fn variable_declarations_are_indexed_without_if_let_noise() {
        assert_eq!(
            declaration_name("const LIMIT: usize = 4;", "main.rs").as_deref(),
            Some("LIMIT")
        );
        assert_eq!(
            declaration_name("pub static mut COUNTER: u32 = 0;", "main.rs").as_deref(),
            Some("COUNTER")
        );
        assert_eq!(
            declaration_name("let mut total = 0;", "main.rs").as_deref(),
            Some("total")
        );
        assert_eq!(
            declaration_name("if let Some(value) = cache.get(key) {", "main.rs"),
            None
        );
        assert_eq!(declaration_name("for name in names {", "main.rs"), None);
    }

    #[test]
    fn declaration_kinds_separate_types_functions_and_variables() {
        assert_eq!(
            declaration_kind("pub struct Engine {", "engine.rs"),
            SymbolKind::Class
        );
        assert_eq!(
            declaration_kind("pub fn run() {", "engine.rs"),
            SymbolKind::Function
        );
        assert_eq!(
            declaration_kind("const LIMIT: usize = 4;", "engine.rs"),
            SymbolKind::Variable
        );
        assert_eq!(
            declaration_kind("class Engine:", "engine.py"),
            SymbolKind::Class
        );
        assert_eq!(
            declaration_kind("// fn hidden() {", "engine.rs"),
            SymbolKind::Other
        );
    }

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
    fn cpp_semantic_categories_are_visually_distinct() {
        let tokens = syntax_tokens(
            "#include <iostream>\nint main() { return LIMIT + 42; }",
            "main.cpp",
        );
        assert!(tokens
            .iter()
            .any(|token| token.kind == TokenKind::Preprocessor));
        assert!(tokens
            .iter()
            .any(|token| token.text == "int" && token.kind == TokenKind::Type));
        assert!(tokens
            .iter()
            .any(|token| token.text == "main" && token.kind == TokenKind::Function));
        assert!(tokens
            .iter()
            .any(|token| token.text == "LIMIT" && token.kind == TokenKind::Constant));
        assert!(tokens
            .iter()
            .any(|token| token.text == "42" && token.kind == TokenKind::Literal));
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

    #[test]
    fn a_line_is_classified_into_the_kinds_the_editor_colours() {
        let kinds = |line: &str| {
            syntax_tokens(line, "main.rs")
                .into_iter()
                .map(|token| (token.kind, token.text))
                .collect::<Vec<_>>()
        };

        // A local binding, its use, a member call and a macro all differ.
        let line = kinds("let mut map = HashMap::new();");
        assert!(line.contains(&(TokenKind::Keyword, "let".into())));
        assert!(line.contains(&(TokenKind::Variable, "map".into())));
        assert!(line.contains(&(TokenKind::Type, "HashMap".into())));
        assert!(line.contains(&(TokenKind::Function, "new".into())));

        // A macro is not a plain name.
        let line = kinds("println!(\"{}\", x);");
        assert!(line.contains(&(TokenKind::Macro, "println".into())));

        // A parameter is not confused with a local.
        let line = kinds("fn build(&self, count: usize) {");
        assert!(line.contains(&(TokenKind::Function, "build".into())));
        assert!(line.contains(&(TokenKind::Parameter, "count".into())));
        assert!(line.contains(&(TokenKind::Type, "usize".into())));

        // A module path names a module, and a member names a property.
        let line = kinds("use std::collections::HashMap;");
        assert!(line.contains(&(TokenKind::Module, "std".into())));

        let line = kinds("    self.session.tick();");
        assert!(line.contains(&(TokenKind::Property, "session".into())));
        assert!(line.contains(&(TokenKind::Function, "tick".into())));

        // A constant stays a constant, and a comment stays a comment.
        let line = kinds("const MAX_WIDTH: usize = 80; // limit");
        assert!(line.contains(&(TokenKind::Constant, "MAX_WIDTH".into())));
        assert!(line.contains(&(TokenKind::Comment, "// limit".into())));
    }

    #[test]
    fn the_token_kinds_are_all_distinct_in_both_themes() {
        use crate::theme_core::{contrast_ratio, Palette, TEXT_MINIMUM};
        for palette in [Palette::dark(), Palette::light()] {
            let colours = [
                palette.keyword,
                palette.syntax_type,
                palette.function,
                palette.constant,
                palette.variable,
                palette.parameter,
                palette.property,
                palette.macro_name,
                palette.module,
                palette.literal,
                palette.comment,
            ];
            for colour in colours {
                // Every syntax colour has to be readable on the editor surface.
                assert!(
                    contrast_ratio(colour, palette.background) >= TEXT_MINIMUM,
                    "#{colour:06x} is too dark on #{:06x}",
                    palette.background
                );
            }
            // The kinds that carry meaning are told apart from each other.
            assert_ne!(palette.variable, palette.function);
            assert_ne!(palette.variable, palette.syntax_type);
            assert_ne!(palette.property, palette.function);
            assert_ne!(palette.macro_name, palette.keyword);
            assert_ne!(palette.constant, palette.literal);
            // A parameter is quieter than a local, but still a distinct colour.
            assert_ne!(palette.parameter, palette.variable);
        }
        // And the two themes really are inverses, not the same table.
        assert_ne!(Palette::dark().variable, Palette::light().variable);
        assert_ne!(Palette::dark().keyword, Palette::light().keyword);
    }
}

/// Text of one line that fits between two columns of the code area.
///
/// The macOS backend draws its code column through this, the other two have
/// their own clipping, so it is allowed to be unused on Linux and Windows.
#[allow(dead_code)]
///
/// A long line must not be drawn over the panel beside it, and every backend
/// has to cut it at the same place, so the rule lives here: take the columns
/// from the first visible one up to the last that still fits, and hand back
/// what to draw along with the column it starts at. A line that is entirely
/// past the right edge returns nothing.
pub fn visible_slice(line: &str, first_column: usize, columns: usize) -> Option<(usize, String)> {
    if columns == 0 {
        return None;
    }
    let characters = line.chars().count();
    if first_column >= characters {
        return None;
    }
    let last = (first_column + columns).min(characters);
    Some((
        first_column,
        line.chars()
            .skip(first_column)
            .take(last - first_column)
            .collect(),
    ))
}

/// Whether a line is wider than the code area, so the backend can say so.
#[allow(dead_code)]
pub fn line_overflows(line: &str, first_column: usize, columns: usize) -> bool {
    line.chars().count() > first_column + columns
}

#[cfg(test)]
mod clipping_tests {
    use super::*;

    #[test]
    fn a_line_is_cut_where_the_code_area_ends() {
        // A short line is handed back whole.
        assert_eq!(
            visible_slice("hello", 0, 80),
            Some((0, "hello".to_string()))
        );
        // A line wider than the area is cut at the last column that fits.
        assert_eq!(
            visible_slice("abcdefghij", 0, 4),
            Some((0, "abcd".to_string()))
        );
        // Scrolled right, the cut follows the view.
        assert_eq!(
            visible_slice("abcdefghij", 3, 4),
            Some((3, "defg".to_string()))
        );
        // The end of the line is not padded.
        assert_eq!(visible_slice("abc", 1, 40), Some((1, "bc".to_string())));

        // Nothing to draw when the line is past the edge or out of columns.
        assert_eq!(visible_slice("abc", 3, 40), None);
        assert_eq!(visible_slice("abc", 99, 40), None);
        assert_eq!(visible_slice("abc", 0, 0), None);
        assert_eq!(visible_slice("", 0, 40), None);

        // What comes back always fits in the columns it was given.
        for width in 1..12usize {
            for first in 0..12usize {
                if let Some((start, text)) = visible_slice("abcdefghijkl", first, width) {
                    assert_eq!(start, first);
                    assert!(
                        text.chars().count() <= width,
                        "{text:?} does not fit in {width}"
                    );
                }
            }
        }
    }

    #[test]
    fn overflow_is_reported_only_when_the_line_is_cut() {
        assert!(!line_overflows("abc", 0, 10));
        assert!(!line_overflows("abcdefghij", 0, 10));
        assert!(line_overflows("abcdefghijk", 0, 10));
        // Scrolled right, the part already behind the edge no longer counts.
        assert!(!line_overflows("abcdefghijk", 1, 10));
        assert!(line_overflows("abcdefghijk", 0, 0));
    }
}
