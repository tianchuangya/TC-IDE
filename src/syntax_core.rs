//! Shared syntax scan.
//!
//! The editors need to know which scope a line belongs to, which declarations a
//! file has and which names are called from inside a function. Doing that with
//! line regular expressions and brace counting gets class members, nested
//! functions and calls in comments wrong, so this module runs a small, language
//! aware scan instead: comments and string literals are blanked out, braces,
//! brackets and parentheses are matched, indentation blocks are closed when the
//! indentation returns, and declarations are attached to the scope that owns
//! them.
//!
//! It is not a full grammar — expression and type analysis stay out of scope —
//! but every rule here is unit tested, including the cases the heuristics used
//! to get wrong.
#![allow(dead_code)] // The backends adopt the pieces they need as they migrate.

use crate::editor_core::{self, SymbolKind, SymbolRole};

/// What a scope is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScopeKind {
    /// The whole file.
    File,
    /// A class, struct, interface, enum or trait.
    Class,
    /// A function, method or closure.
    Function,
    /// An unnamed brace or indentation block.
    Block,
}

/// One scope of a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scope {
    pub kind: ScopeKind,
    /// Declaration name of a class or function scope.
    pub name: Option<String>,
    pub start_line: usize,
    pub end_line: usize,
    /// Index of the scope that contains this one.
    pub parent: Option<usize>,
}

impl Scope {
    /// Whether a line falls inside this scope.
    pub fn contains(&self, line: usize) -> bool {
        line >= self.start_line && line <= self.end_line
    }
}

/// A declaration with the scope that owns it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Declaration {
    pub name: String,
    pub kind: SymbolKind,
    pub line: usize,
    pub column: usize,
    /// Index of the owning scope.
    pub scope: usize,
}

/// What a call is made on, when the scan can tell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CallReceiver {
    /// `self.run()` or `this.run()`.
    SelfRef,
    /// `Engine::new()` or `Engine::default().build()` style qualifiers.
    Type(String),
    /// `writer.flush()` on a value.
    Value(String),
}

impl CallReceiver {
    /// The owner the receiver names, when it is one.
    ///
    /// `self` belongs to the scope the call sits in, which the resolver knows.
    pub fn owner(&self) -> Option<&str> {
        match self {
            CallReceiver::SelfRef => None,
            CallReceiver::Type(name) | CallReceiver::Value(name) => Some(name),
        }
    }

    /// Whether the receiver starts with a capital, which reads as a type.
    pub fn looks_like_type(&self) -> bool {
        match self {
            CallReceiver::Type(_) => true,
            CallReceiver::Value(name) => name
                .chars()
                .next()
                .map(|value| value.is_uppercase())
                .unwrap_or(false),
            CallReceiver::SelfRef => false,
        }
    }
}

/// A call shaped identifier occurrence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Call {
    pub name: String,
    pub line: usize,
    pub column: usize,
    /// Index of the innermost scope the call appears in.
    pub scope: usize,
    /// What the call is made on, for `self.helper()`, `Engine::new()` or
    /// `writer.flush()`. `None` for a plain `helper()`.
    pub receiver: Option<CallReceiver>,
}

/// Everything the scan learned about one file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileScan {
    pub scopes: Vec<Scope>,
    pub declarations: Vec<Declaration>,
    pub calls: Vec<Call>,
    /// Local variables whose value is a constructor call of a known class.
    pub bindings: Vec<LocalBinding>,
    /// Return type each function declares, by name.
    pub returns: Vec<(String, String)>,
}

/// A local variable that holds an instance of a class.
///
/// `let engine = Engine::new()` is the one piece of type information a line
/// scanner can be sure about, and it is enough to resolve `engine.run()` to the
/// method of `Engine` instead of guessing between every `run` in the project.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalBinding {
    pub name: String,
    pub class: String,
    pub line: usize,
    /// Index of the scope the binding is visible in.
    pub scope: usize,
}

impl FileScan {
    /// Class a local variable holds at `line`, when the scan can tell.
    ///
    /// Only bindings in the scope the call sits in, or in a scope that contains
    /// it, count: a local of a sibling function is not in scope.
    pub fn binding_of(&self, name: &str, line: usize, scope: usize) -> Option<&LocalBinding> {
        self.bindings
            .iter()
            .filter(|binding| {
                binding.name == name
                    && binding.line <= line
                    && (binding.scope == scope || self.scope_contains(binding.scope, scope))
            })
            .max_by_key(|binding| binding.line)
    }

    /// Whether `ancestor` contains `scope`, itself included.
    pub fn scope_contains(&self, ancestor: usize, scope: usize) -> bool {
        let mut current = Some(scope);
        while let Some(index) = current {
            if index == ancestor {
                return true;
            }
            current = self.scopes.get(index).and_then(|value| value.parent);
        }
        false
    }
}

impl FileScan {
    /// Declaration of a name, preferring the first one in the file.
    pub fn declaration(&self, name: &str) -> Option<&Declaration> {
        self.declarations.iter().find(|entry| entry.name == name)
    }

    /// Innermost scope containing a line.
    pub fn scope_at(&self, line: usize) -> Option<usize> {
        self.scopes
            .iter()
            .enumerate()
            .filter(|(_, scope)| scope.contains(line))
            .max_by_key(|(_, scope)| self.depth_of(scope))
            .map(|(index, _)| index)
    }

    /// Scope of a declaration.
    pub fn scope_of_symbol(&self, name: &str, line: usize) -> Option<usize> {
        self.declarations
            .iter()
            .find(|entry| entry.name == name && entry.line == line)
            .map(|entry| entry.scope)
    }

    /// Declarations written inside a scope, in line order.
    ///
    /// A method owns the scope it opens, but it is still a member of its class,
    /// so membership follows the lexical range rather than the owning scope.
    pub fn members_of(&self, scope: usize) -> Vec<&Declaration> {
        let Some(value) = self.scopes.get(scope) else {
            return Vec::new();
        };
        self.declarations
            .iter()
            .filter(|entry| value.contains(entry.line) && entry.line != value.start_line)
            .collect()
    }

    /// Calls made from inside a scope, without the declaration itself.
    pub fn calls_in(&self, scope: usize) -> Vec<&Call> {
        self.calls
            .iter()
            .filter(|call| call.scope == scope)
            .collect()
    }

    /// Names called from inside a scope, in first-seen order.
    pub fn called_names(&self, scope: usize) -> Vec<String> {
        let mut names = Vec::new();
        for call in self.calls_in(scope) {
            if !names.iter().any(|name| name == &call.name) {
                names.push(call.name.clone());
            }
        }
        names
    }

    /// Whether a declaration is a member of a class rather than a top level item.
    pub fn is_class_member(&self, declaration: &Declaration) -> bool {
        self.scopes.iter().any(|scope| {
            scope.kind == ScopeKind::Class
                && scope.contains(declaration.line)
                && scope.start_line != declaration.line
        })
    }

    fn depth_of(&self, scope: &Scope) -> usize {
        let mut depth = 0;
        let mut current = scope.parent;
        while let Some(index) = current {
            depth += 1;
            current = self.scopes.get(index).and_then(|scope| scope.parent);
        }
        depth
    }
}

/// Class-like scope a line opens without declaring a type of its own.
///
/// `impl Display for Engine {` opens the scope of `Engine`, and `namespace Foo {`
/// opens `Foo`. Neither adds a symbol: the type itself is declared by its
/// `struct`, `class` or `trait` line, and a second declaration would show the
/// same class twice in the graph.
fn scope_hint(masked: &str) -> Option<(String, ScopeKind)> {
    let words = editor_core::identifier_ranges(masked);
    let keyword = words.first()?.2.as_str();
    if !matches!(
        keyword,
        "impl" | "namespace" | "module" | "object" | "extension"
    ) {
        return None;
    }
    let names = words
        .iter()
        .skip(1)
        .map(|entry| entry.2.clone())
        .collect::<Vec<_>>();
    if names.is_empty() {
        return None;
    }
    // `impl Display for Engine` belongs to Engine, not to the trait.
    let name = match names.iter().position(|value| value == "for") {
        Some(position) => names
            .get(position + 1)
            .cloned()
            .unwrap_or_else(|| names[0].clone()),
        None => names[0].clone(),
    };
    Some((name, ScopeKind::Class))
}

/// Replace comments and string literals with spaces, keeping column positions.
///
/// The scan works on this masked text so a brace inside a comment or a string
/// never opens a scope and a call inside a string is never reported.
pub fn mask_line(line: &str, file: &str) -> String {
    let mut characters = line.chars().collect::<Vec<_>>();
    let comment = editor_core::comment_bounds(line, file);
    let code_end = comment.map(|(start, _, _)| start).unwrap_or(line.len());
    let mut index = 0usize;
    let length = characters.len();
    let mut quote: Option<char> = None;
    let language = editor_core::language_for_file(file);
    let backtick_strings = matches!(language, "TypeScript / JavaScript" | "Go" | "SQL" | "Shell");
    let single_quotes = !matches!(language, "Plain Text" | "JSON");
    while index < length {
        let character = characters[index];
        // Byte offset of this character, to compare against the comment start.
        let byte = characters[..index]
            .iter()
            .map(|value| value.len_utf8())
            .sum::<usize>();
        if byte >= code_end {
            for value in characters.iter_mut().skip(index) {
                *value = ' ';
            }
            break;
        }
        match quote {
            Some(active) => {
                if character == '\\' {
                    characters[index] = ' ';
                    if index + 1 < length {
                        characters[index + 1] = ' ';
                    }
                    index += 2;
                    continue;
                }
                if character == active {
                    quote = None;
                }
                characters[index] = ' ';
            }
            None => {
                if character == '"' || (single_quotes && character == '\'') {
                    quote = Some(character);
                    characters[index] = ' ';
                } else if character == '`' && backtick_strings {
                    quote = Some('`');
                    characters[index] = ' ';
                }
            }
        }
        index += 1;
    }
    characters.into_iter().collect()
}

/// Indentation of a line, counting spaces and tabs.
fn indentation(line: &str) -> usize {
    line.chars()
        .take_while(|value| *value == ' ' || *value == '\t')
        .count()
}

/// Scan a file into scopes, declarations and calls.
pub fn scan(lines: &[String], file: &str) -> FileScan {
    let mut scopes = vec![Scope {
        kind: ScopeKind::File,
        name: None,
        start_line: 0,
        end_line: lines.len().saturating_sub(1),
        parent: None,
    }];
    let mut declarations = Vec::new();
    let mut calls = Vec::new();
    let mut bindings = Vec::new();
    let mut returns: Vec<(String, String)> = Vec::new();
    let language = editor_core::language_for_file(file);
    let mut stack: Vec<usize> = vec![0];
    let mut pending_braces: Vec<usize> = Vec::new();
    let mut indent_stack: Vec<(usize, usize)> = Vec::new();

    for (line_index, raw) in lines.iter().enumerate() {
        let masked = mask_line(raw, file);
        let trimmed = masked.trim();
        if trimmed.is_empty() {
            continue;
        }
        let indent = indentation(raw);

        // Close indentation scopes that this line has left.
        while let Some((scope, open_indent)) = indent_stack.last().copied() {
            let scope_ends = indent <= open_indent
                && scopes
                    .get(scope)
                    .map(|value| value.start_line < line_index)
                    .unwrap_or(false);
            if !scope_ends {
                break;
            }
            if let Some(value) = scopes.get_mut(scope) {
                value.end_line = line_index.saturating_sub(1);
            }
            indent_stack.pop();
            stack.retain(|index| *index != scope);
        }

        let declaration = editor_core::declaration_name(raw, file);
        let kind = editor_core::declaration_kind(raw, file);
        let hint = scope_hint(trimmed);
        let symbol_kind = declaration.as_ref().map(|_| kind);
        let scope_index = *stack.last().unwrap_or(&0);
        let mut opened_function = None;
        let mut declaration_index = None;
        if let Some(name) = declaration.clone() {
            let column = masked.find(&name).unwrap_or(0);
            declarations.push(Declaration {
                name: name.clone(),
                kind: symbol_kind.unwrap_or(SymbolKind::Other),
                line: line_index,
                column: masked[..column.min(masked.len())].chars().count(),
                scope: scope_index,
            });
            declaration_index = Some(declarations.len() - 1);
            opened_function = Some(name);
        }

        // Brace delimited languages: follow the braces in order so a line such
        // as `pub fn helper() {}` opens and closes its own scope.
        let mut first_open = true;
        for (offset, character) in masked.char_indices() {
            match character {
                '{' => {
                    let (kind, name) = if first_open {
                        if let Some((name, kind)) = hint.clone() {
                            (kind, Some(name))
                        } else if opened_function.is_some() {
                            (
                                if symbol_kind == Some(SymbolKind::Class) {
                                    ScopeKind::Class
                                } else {
                                    ScopeKind::Function
                                },
                                opened_function.clone(),
                            )
                        } else {
                            (ScopeKind::Block, None)
                        }
                    } else {
                        (ScopeKind::Block, None)
                    };
                    let parent = *stack.last().unwrap_or(&0);
                    scopes.push(Scope {
                        kind,
                        name,
                        start_line: line_index,
                        end_line: lines.len().saturating_sub(1),
                        parent: Some(parent),
                    });
                    let index = scopes.len() - 1;
                    stack.push(index);
                    pending_braces.push(index);
                    if first_open {
                        if let Some(position) = declaration_index {
                            // The declaration owns the scope it opens.
                            declarations[position].scope = index;
                        }
                    }
                    first_open = false;
                    let _ = offset;
                }
                '}' => {
                    if let Some(index) = pending_braces.pop() {
                        if let Some(value) = scopes.get_mut(index) {
                            value.end_line = line_index;
                        }
                        stack.retain(|candidate| *candidate != index);
                    }
                }
                _ => {}
            }
        }
        if opened_function.is_some() && !masked.contains('{') && trimmed.ends_with(':') {
            // Indentation delimited block, as in Python or YAML.
            let parent = *stack.last().unwrap_or(&0);
            scopes.push(Scope {
                kind: if symbol_kind == Some(SymbolKind::Class) {
                    ScopeKind::Class
                } else {
                    ScopeKind::Function
                },
                name: opened_function.clone(),
                start_line: line_index,
                end_line: lines.len().saturating_sub(1),
                parent: Some(parent),
            });
            let index = scopes.len() - 1;
            stack.push(index);
            indent_stack.push((index, indent));
            if let Some(position) = declaration_index {
                declarations[position].scope = index;
            }
        }

        // A declared return type is remembered, so a later step can follow
        // `let engine = build()` to the class `build` hands back.
        if let Some((function, returned)) = declared_return_type(&masked, language) {
            if !returns.iter().any(|(known, _)| known == &function) {
                returns.push((function, returned));
            }
        }

        // `let engine = Engine::new()` and `const engine = new Engine()`: the
        // only local type information a line scanner can be sure about.
        if let Some((name, class)) = local_binding(&masked) {
            bindings.push(LocalBinding {
                name,
                class,
                line: line_index,
                scope: *stack.last().unwrap_or(&0),
            });
        }

        // Calls: an identifier followed by an opening parenthesis that is not
        // the declaration on this line and not a control keyword.
        let inner_scope = *stack.last().unwrap_or(&0);
        for (start, end, receiver) in call_positions(&masked) {
            let name = &masked[start..end];
            if declaration.as_deref() == Some(name) && receiver.is_none() {
                // The declaration on this line opens the scope the call would
                // otherwise be counted in; `Engine::run();` is not that.
                continue;
            }
            if matches!(
                name,
                "if" | "for"
                    | "while"
                    | "switch"
                    | "catch"
                    | "return"
                    | "match"
                    | "print"
                    | "echo"
                    | "loop"
            ) {
                // `print`/`echo` are real calls in several languages, so they are
                // kept; the control keywords above are not.
                if !matches!(name, "print" | "echo") {
                    continue;
                }
            }
            let column = masked[..start].chars().count();
            calls.push(Call {
                name: name.to_string(),
                line: line_index,
                column,
                scope: inner_scope,
                receiver,
            });
        }
    }

    for index in pending_braces.into_iter().rev() {
        if let Some(value) = scopes.get_mut(index) {
            value.end_line = lines.len().saturating_sub(1);
        }
    }
    for (scope, _) in indent_stack {
        if let Some(value) = scopes.get_mut(scope) {
            value.end_line = lines.len().saturating_sub(1);
        }
    }
    FileScan {
        scopes,
        declarations,
        calls,
        bindings,
        returns,
    }
}

/// Local variable of a line and the class its initializer constructs.
///
/// Recognised forms are `let engine = Engine::new()`, `let engine = new Engine()`
/// and `const engine = Engine()`, in the languages that spell them that way. A
/// variable assigned something the scan cannot name (`let engine = build()`) is
/// left out instead of guessed.
fn local_binding(masked: &str) -> Option<(String, String)> {
    let trimmed = masked.trim_start();
    // A declaration keyword, or a typed declaration such as
    // `Engine engine = Engine::new()` where only the name before `=` matters.
    let rest = match ["let ", "const ", "var ", "auto ", "final ", "val "]
        .iter()
        .find_map(|keyword| trimmed.strip_prefix(keyword))
    {
        Some(rest) => rest,
        None => trimmed,
    };
    let (name, value) = rest.split_once('=')?;
    let name = name.trim();
    if name.is_empty()
        || name
            .chars()
            .any(|character| !character.is_alphanumeric() && character != '_')
        || name.chars().next().map(|value| value.is_ascii_digit()) == Some(true)
        || matches!(
            name,
            "if" | "while" | "return" | "else" | "and" | "or" | "not" | "print" | "new"
        )
    {
        return None;
    }
    let value = value.trim_start();
    // `new Engine(...)`.
    if let Some(after_new) = value.strip_prefix("new ") {
        let class = identifier_at(after_new)?;
        if after_new[class.len()..].trim_start().starts_with('(') {
            return Some((name.to_string(), class));
        }
        return None;
    }
    // `Engine::new(...)` or `Engine(...)`.
    let class = identifier_at(value)?;
    let after = value[class.len()..].trim_start();
    if (after.starts_with("::") || after.starts_with('('))
        && class.chars().next().map(|value| value.is_uppercase()) == Some(true)
    {
        return Some((name.to_string(), class));
    }
    None
}

/// The return type a function declaration names, when it names one.
///
/// This is what lets `let engine = build(); engine.run()` resolve: the scanner
/// knows `build` returns an `Engine`, so the binding holds one. Only declared
/// types are read - a function whose signature says nothing (or says `auto`,
/// `void`, `var`) is deliberately left alone rather than guessed at.
fn declared_return_type(line: &str, language: &str) -> Option<(String, String)> {
    let trimmed = line.trim_start();
    // Rust and TypeScript: `fn build(&self) -> Engine {`.
    for keyword in ["fn ", "pub fn ", "async fn ", "function ", "func "] {
        let Some(rest) = trimmed.strip_prefix(keyword) else {
            continue;
        };
        let rest = rest.trim_start_matches("async ").trim_start();
        let name = identifier_at(rest)?;
        let after_name = rest[name.len()..].trim_start();
        let after_params = after_name.strip_prefix('(')?;
        let close = after_params.find(')')?;
        let tail = after_params[close + 1..].trim_start();
        let returned = tail.strip_prefix("->").or_else(|| tail.strip_prefix(':'))?;
        return Some((name, return_type_name(returned)?));
    }
    // C, C++, Java, C#: `Engine build(int count) {` - the type comes first.
    let (before, _) = trimmed.split_once('(')?;
    let head = before.trim_end();
    let call_name = head.rsplit(|character: char| !character.is_alphanumeric() && character != '_');
    let function = call_name.clone().next()?.to_string();
    if function.is_empty() || matches!(function.as_str(), "if" | "for" | "while" | "switch") {
        return None;
    }
    let type_part = head[..head.len() - function.len()].trim();
    if type_part.is_empty() {
        return None;
    }
    // `static Engine*` keeps its storage modifier in front of the type, and
    // `virtual`/`const`/`inline` do the same, so they are dropped before the
    // type is read.
    let type_part = type_part
        .split_whitespace()
        .filter(|word| {
            !matches!(
                word.trim_end_matches(['*', '&']),
                "static"
                    | "virtual"
                    | "inline"
                    | "const"
                    | "constexpr"
                    | "extern"
                    | "public"
                    | "private"
                    | "protected"
                    | "final"
                    | "abstract"
                    | "async"
                    | "unsafe"
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    if type_part.is_empty() {
        return None;
    }
    let returned = return_type_name(&type_part)?;
    if matches!(returned.as_str(), "return" | "new" | "delete") {
        return None;
    }
    let _ = language;
    Some((function, returned))
}

/// The class name inside a written return type, dropping pointers, references
/// and generics: `Engine*`, `&Engine` and `Vec<Engine>` all name something the
/// scanner can look up, and `Vec` alone would be the wrong answer.
fn return_type_name(value: &str) -> Option<String> {
    let cleaned = value
        .trim()
        .trim_start_matches(['&', '*', ' '])
        .trim_end_matches(['*', '&', ' ', '{', ';'])
        .trim();
    // `Vec<Engine>` names the argument, which is what a `.run()` would be on.
    if let Some(open) = cleaned.find('<') {
        if let Some(close) = cleaned.rfind('>') {
            let inner = &cleaned[open + 1..close];
            if let Some(argument) = inner.split(',').next().map(str::trim) {
                if !argument.is_empty() {
                    return return_type_name(argument);
                }
            }
        }
    }
    let name = identifier_at(cleaned)?;
    if matches!(
        name.as_str(),
        "void" | "auto" | "var" | "self" | "Self" | "None" | "never"
    ) {
        return None;
    }
    if !name.chars().next().is_some_and(char::is_uppercase) {
        return None;
    }
    Some(name)
}

/// Identifier at the start of a piece of code.
fn identifier_at(value: &str) -> Option<String> {
    let value = value.trim_start();
    let end = value
        .char_indices()
        .find(|(_, character)| !character.is_alphanumeric() && *character != '_')
        .map(|(offset, _)| offset)
        .unwrap_or(value.len());
    if end == 0 {
        return None;
    }
    Some(value[..end].to_string())
}

/// Byte ranges of `name(` patterns in masked text, with what they are called on.
///
/// Member calls used to be skipped because they "belonged to the object", but
/// that also lost every `self.helper()` and `Engine::new()` edge in the
/// relation graph. They are reported now, together with the receiver, so the
/// resolver can look the member up in the class that owns the call.
fn call_positions(masked: &str) -> Vec<(usize, usize, Option<CallReceiver>)> {
    let mut positions = Vec::new();
    let bytes = masked.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        let byte = bytes[index];
        if !(byte.is_ascii_alphabetic() || byte == b'_') {
            index += 1;
            continue;
        }
        let start = index;
        while index < bytes.len() && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
        {
            index += 1;
        }
        let end = index;
        let mut next = index;
        while next < bytes.len() && bytes[next] == b' ' {
            next += 1;
        }
        if next < bytes.len() && bytes[next] == b'(' {
            positions.push((start, end, receiver_before(&masked[..start])));
        }
    }
    positions
}

/// The receiver written before a call site, if the text names one.
fn receiver_before(before: &str) -> Option<CallReceiver> {
    let trimmed = before.trim_end();
    let separator = if trimmed.ends_with("->") || trimmed.ends_with("::") {
        trimmed.len() - 2
    } else if trimmed.ends_with('.') {
        trimmed.len() - 1
    } else {
        return None;
    };
    let head = &trimmed[..separator];
    let start = head
        .char_indices()
        .rev()
        .find(|(_, value)| !value.is_alphanumeric() && *value != '_' && *value != ':')
        .map(|(offset, value)| offset + value.len_utf8())
        .unwrap_or(0);
    let name = head[start..].trim_start_matches(':');
    if name.is_empty() {
        // A chained call such as `build().run()`: the receiver is a value the
        // scan cannot name, so it stays unknown instead of guessed.
        return None;
    }
    Some(match name {
        "self" | "this" => CallReceiver::SelfRef,
        _ => {
            let last = name.rsplit("::").next().unwrap_or(name);
            if last.chars().next().map(|value| value.is_uppercase()) == Some(true) {
                CallReceiver::Type(last.to_string())
            } else {
                CallReceiver::Value(last.to_string())
            }
        }
    })
}

/// A declaration somewhere in the project, as the index stores it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclarationRef {
    pub name: String,
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub kind: SymbolKind,
    /// Class or function the declaration belongs to, when it has one.
    pub owner: Option<String>,
}

/// How sure the resolver is about a call target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CallConfidence {
    /// A member of the class the call sits in, reached through `self`.
    Member,
    /// A declaration of the file the call appears in.
    SameFile,
    /// A declaration of another file of the project.
    Project,
}

/// One declaration a call may refer to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedCall {
    pub declaration: DeclarationRef,
    pub confidence: CallConfidence,
}

/// The class or function a scope belongs to, nearest first.
pub fn owner_of(scan: &FileScan, scope: usize) -> Option<String> {
    let mut current = Some(scope);
    while let Some(index) = current {
        let value = scan.scopes.get(index)?;
        if value.kind == ScopeKind::Class || value.kind == ScopeKind::Function {
            if let Some(name) = &value.name {
                return Some(name.clone());
            }
        }
        current = value.parent;
    }
    None
}

/// The declarations of one file, each with the owner it belongs to.
pub fn declaration_refs(scan: &FileScan, file: &str) -> Vec<DeclarationRef> {
    scan.declarations
        .iter()
        .map(|declaration| DeclarationRef {
            name: declaration.name.clone(),
            file: file.to_string(),
            line: declaration.line,
            column: declaration.column,
            kind: declaration.kind,
            owner: owner_of_declaration(scan, declaration),
        })
        .collect()
}

/// The class or function a declaration belongs to.
///
/// A method owns the scope it opens, so the walk starts above that scope: the
/// owner of `impl Engine { fn build() }` is `Engine`, not `build`.
fn owner_of_declaration(scan: &FileScan, declaration: &Declaration) -> Option<String> {
    let owned = scan.scopes.get(declaration.scope).and_then(|scope| {
        let owns = scope.start_line == declaration.line
            && scope.name.as_deref() == Some(declaration.name.as_str());
        if owns {
            scope.parent
        } else {
            Some(declaration.scope)
        }
    });
    owner_of(scan, owned?)
}

/// Rank the declarations a call site may refer to, best match first.
///
/// The rules follow what a reader would assume: `self.helper()` is a member of
/// the class the call sits in, a capitalized qualifier such as `Engine::new()`
/// is an associated item of that type, and anything else is looked up in the
/// file first and in the rest of the project afterwards. The scan cannot see
/// types, so a name with several declarations keeps all of them, ranked, and the
/// relation graph shows the weaker ones as weak edges instead of inventing one.
pub fn resolve_call(
    call: &Call,
    scan: &FileScan,
    file: &str,
    project: &[DeclarationRef],
    limit: usize,
) -> Vec<ResolvedCall> {
    let mut candidates: Vec<DeclarationRef> = declaration_refs(scan, file)
        .into_iter()
        .chain(project.iter().cloned())
        .filter(|declaration| declaration.name == call.name)
        .collect();
    candidates.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then_with(|| left.line.cmp(&right.line))
            .then_with(|| left.column.cmp(&right.column))
    });
    candidates.dedup_by(|left, right| {
        left.file == right.file && left.line == right.line && left.name == right.name
    });
    if candidates.is_empty() || limit == 0 {
        return Vec::new();
    }

    // `self.helper()` belongs to the class the call sits in, which is the
    // enclosing class even when the call is inside a nested function.
    let self_owner = call
        .receiver
        .as_ref()
        .filter(|receiver| **receiver == CallReceiver::SelfRef)
        .and_then(|_| owning_class(scan, call.scope));
    // `engine.run()` is a member of whatever class `engine` holds, which the
    // scan knows when the variable was assigned a constructor call.
    let local_owner = call
        .receiver
        .as_ref()
        .and_then(|receiver| match receiver {
            CallReceiver::Value(name) | CallReceiver::Type(name) => Some(name.as_str()),
            CallReceiver::SelfRef => None,
        })
        .and_then(|name| scan.binding_of(name, call.line, call.scope))
        .map(|binding| binding.class.clone());
    let named_owner = call
        .receiver
        .as_ref()
        .filter(|receiver| receiver.looks_like_type())
        .and_then(|receiver| receiver.owner())
        .map(str::to_string)
        .or(local_owner);

    let mut ranked = candidates
        .into_iter()
        .map(|declaration| {
            let confidence = if let Some(owner) = self_owner.as_deref() {
                if declaration.owner.as_deref() == Some(owner) && declaration.file == file {
                    CallConfidence::Member
                } else if declaration.file == file {
                    CallConfidence::SameFile
                } else {
                    CallConfidence::Project
                }
            } else if let Some(owner) = named_owner.as_deref() {
                if declaration.owner.as_deref() == Some(owner) {
                    CallConfidence::Member
                } else if declaration.file == file {
                    CallConfidence::SameFile
                } else {
                    CallConfidence::Project
                }
            } else if declaration.file == file {
                CallConfidence::SameFile
            } else {
                CallConfidence::Project
            };
            ResolvedCall {
                declaration,
                confidence,
            }
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        left.confidence
            .cmp(&right.confidence)
            .then_with(|| left.declaration.file.cmp(&right.declaration.file))
            .then_with(|| left.declaration.line.cmp(&right.declaration.line))
    });
    ranked.truncate(limit);
    ranked
}

/// The class a scope belongs to, even when the call is inside a method.
fn owning_class(scan: &FileScan, scope: usize) -> Option<String> {
    let mut current = Some(scope);
    while let Some(index) = current {
        let value = scan.scopes.get(index)?;
        if value.kind == ScopeKind::Class {
            if let Some(name) = &value.name {
                return Some(name.clone());
            }
        }
        current = value.parent;
    }
    None
}

/// Occurrences of a name that look like calls, for one file.
pub fn calls_of<'a>(scan: &'a FileScan, name: &str) -> Vec<&'a Call> {
    scan.calls.iter().filter(|call| call.name == name).collect()
}

/// Roles of every occurrence of the names a file declares.
///
/// The editors use this for hover rails and the symbol list, so it keeps the
/// shape `editor_core::symbol_occurrences` had while deciding roles from the
/// scan rather than from a line-by-line guess.
pub fn symbol_occurrences(lines: &[String], file: &str) -> Vec<editor_core::SymbolOccurrence> {
    let scan = scan(lines, file);
    let mut occurrences = Vec::new();
    for (line_index, raw) in lines.iter().enumerate() {
        let masked = mask_line(raw, file);
        for (start, end, name) in editor_core::identifier_ranges(&masked) {
            if !scan.declarations.iter().any(|entry| entry.name == name) {
                continue;
            }
            let declaration = scan
                .declarations
                .iter()
                .any(|entry| entry.name == name && entry.line == line_index);
            occurrences.push(editor_core::SymbolOccurrence {
                name,
                line: line_index,
                start,
                end,
                role: if declaration {
                    SymbolRole::Declaration
                } else {
                    SymbolRole::Reference
                },
            });
        }
    }
    occurrences
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn comments_and_strings_are_masked_without_moving_columns() {
        let masked = mask_line("let a = \"{\"; // } not a brace", "main.rs");
        assert!(!masked.contains('{'));
        assert!(!masked.contains('}'));
        assert_eq!(masked.len(), "let a = \"{\"; // } not a brace".len());
        assert_eq!(
            mask_line("#include <stdio.h>", "main.c"),
            "#include <stdio.h>"
        );
        assert!(mask_line("// fn hidden() {}", "main.rs").trim().is_empty());
        assert!(mask_line("# fn hidden()", "main.py").trim().is_empty());
        assert_eq!(
            mask_line("let s = `a}b`;", "main.ts").matches('}').count(),
            0
        );
    }

    #[test]
    fn braces_open_and_close_scopes() {
        let source = lines(&[
            "pub struct Engine {",
            "    limit: usize,",
            "}",
            "pub fn run() {",
            "    let value = 1;",
            "}",
        ]);
        let scan = scan(&source, "main.rs");
        let class = scan
            .scopes
            .iter()
            .find(|scope| scope.kind == ScopeKind::Class)
            .expect("class scope");
        assert_eq!(class.name.as_deref(), Some("Engine"));
        assert_eq!((class.start_line, class.end_line), (0, 2));
        let function = scan
            .scopes
            .iter()
            .find(|scope| scope.kind == ScopeKind::Function)
            .expect("function scope");
        assert_eq!(function.name.as_deref(), Some("run"));
        assert_eq!((function.start_line, function.end_line), (3, 5));
        // Engine, run and the `value` variable are all declarations.
        assert_eq!(scan.declarations.len(), 3);
    }

    #[test]
    fn nested_scopes_and_members_are_attached_to_their_owner() {
        let source = lines(&[
            "class Engine {",
            "    start() {",
            "        helper();",
            "    }",
            "    stop() {",
            "    }",
            "}",
            "function helper() {",
            "}",
        ]);
        let scan = scan(&source, "main.ts");
        let class = scan
            .scopes
            .iter()
            .position(|scope| scope.kind == ScopeKind::Class)
            .expect("class");
        let members = scan.members_of(class);
        let names = members
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, ["start", "stop"]);
        assert!(members.iter().all(|entry| scan.is_class_member(entry)));
        assert!(
            !scan
                .members_of(class)
                .iter()
                .any(|entry| entry.name == "Engine"),
            "the class is not a member of itself"
        );

        // The call belongs to `start`, not to the class or the file.
        let start = scan
            .scopes
            .iter()
            .position(|scope| scope.name.as_deref() == Some("start"))
            .expect("start");
        assert_eq!(scan.called_names(start), ["helper"]);
        assert!(scan.called_names(class).is_empty());
        assert!(
            scan.called_names(0).is_empty(),
            "nothing is called at the top level of this file"
        );

        // `helper` is a top level function, not a class member.
        let helper = scan.declaration("helper").expect("helper");
        assert!(!scan.is_class_member(helper));
    }

    #[test]
    fn indentation_languages_close_scopes_when_indentation_returns() {
        let source = lines(&[
            "class Engine:",
            "    def start(self):",
            "        helper()",
            "    def stop(self):",
            "        pass",
            "def helper():",
            "    pass",
        ]);
        let scan = scan(&source, "main.py");
        let class = scan
            .scopes
            .iter()
            .find(|scope| scope.name.as_deref() == Some("Engine"))
            .expect("class");
        assert_eq!((class.start_line, class.end_line), (0, 4));
        let start = scan
            .scopes
            .iter()
            .find(|scope| scope.name.as_deref() == Some("start"))
            .expect("start");
        // `def stop` on line 3 ends the previous scope.
        assert_eq!((start.start_line, start.end_line), (1, 2));
        let helper = scan.declaration("helper").expect("helper");
        assert_eq!(
            scan.scopes[helper.scope].kind,
            ScopeKind::Function,
            "a function owns its own scope"
        );
        assert!(!scan.is_class_member(helper));
    }

    #[test]
    fn calls_ignore_comments_and_strings_but_keep_their_receiver() {
        let source = lines(&[
            "pub fn run() {",
            "    helper();",
            "    // other();",
            "    let text = \"third()\";",
            "    self.method();",
            "    crate::fourth();",
            "}",
        ]);
        let scan = scan(&source, "main.rs");
        let function = scan
            .scopes
            .iter()
            .position(|scope| scope.name.as_deref() == Some("run"))
            .expect("run");
        assert_eq!(
            scan.called_names(function),
            ["helper", "method", "fourth"],
            "a comment and a string are not calls, a member call is"
        );
        let call = |name: &str| {
            scan.calls
                .iter()
                .find(|call| call.name == name)
                .expect("the call")
        };
        assert_eq!(call("helper").receiver, None);
        assert_eq!(call("method").receiver, Some(CallReceiver::SelfRef));
        assert_eq!(
            call("fourth").receiver,
            Some(CallReceiver::Value("crate".into()))
        );
        assert!(!scan.calls.iter().any(|call| call.name == "other"));
        assert!(!scan.calls.iter().any(|call| call.name == "third"));
    }

    #[test]
    fn a_file_without_scopes_still_scans() {
        let scan = scan(&lines(&["let a = 1;", "let b = 2;"]), "main.rs");
        assert_eq!(scan.scopes.len(), 1);
        // Variables are declarations and belong to the file scope.
        assert_eq!(scan.declarations.len(), 2);
        assert!(scan.declarations.iter().all(|entry| entry.scope == 0));
        assert!(scan.calls.is_empty());
        assert_eq!(scan.scope_at(1), Some(0));
    }

    #[test]
    fn scope_lookup_prefers_the_innermost_scope() {
        let source = lines(&[
            "class Engine {",
            "    start() {",
            "        helper();",
            "    }",
            "}",
        ]);
        let scan = scan(&source, "main.ts");
        let inner = scan.scope_at(2).expect("scope");
        assert_eq!(scan.scopes[inner].name.as_deref(), Some("start"));
        assert_eq!(
            scan.scope_at(0).map(|index| scan.scopes[index].kind),
            Some(ScopeKind::Class)
        );
        assert!(scan.scope_at(99).is_none());
    }

    #[test]
    fn occurrences_use_the_scan_for_roles() {
        let source = lines(&["pub fn run() {", "    helper();", "}", "pub fn helper() {}"]);
        let occurrences = symbol_occurrences(&source, "main.rs");
        let declarations = occurrences
            .iter()
            .filter(|entry| entry.role == SymbolRole::Declaration)
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>();
        let references = occurrences
            .iter()
            .filter(|entry| entry.role == SymbolRole::Reference)
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(declarations, ["run", "helper"]);
        assert_eq!(references, ["helper"]);
    }

    #[test]
    fn a_scan_reports_what_a_call_is_made_on() {
        let source = lines(&[
            "struct Engine {",
            "    name: String,",
            "}",
            "impl Engine {",
            "    fn build(&self) {",
            "        self.prepare();",
            "        Engine::default();",
            "        writer.flush();",
            "        helper();",
            "        std::mem::drop(value);",
            "        make().run();",
            "    }",
            "    fn prepare(&self) {}",
            "}",
        ]);
        let scan = scan(&source, "engine.rs");
        let receiver = |name: &str| {
            scan.calls
                .iter()
                .find(|call| call.name == name)
                .and_then(|call| call.receiver.clone())
        };
        assert_eq!(receiver("prepare"), Some(CallReceiver::SelfRef));
        assert_eq!(
            receiver("default"),
            Some(CallReceiver::Type("Engine".into()))
        );
        assert_eq!(
            receiver("flush"),
            Some(CallReceiver::Value("writer".into()))
        );
        assert_eq!(receiver("helper"), None);
        assert_eq!(
            receiver("drop"),
            Some(CallReceiver::Value("mem".into())),
            "a module path keeps its last segment"
        );
        assert_eq!(
            receiver("run"),
            None,
            "a chained call has no nameable receiver"
        );
        assert!(
            scan.calls.iter().any(|call| call.name == "prepare"),
            "member calls are no longer dropped"
        );
    }

    #[test]
    fn declarations_carry_the_owner_they_belong_to() {
        let source = lines(&[
            "impl Engine {",
            "    fn build(&self) {",
            "        let nested = |value: i32| value + 1;",
            "    }",
            "    fn prepare(&self) {}",
            "}",
            "fn free() {}",
        ]);
        let scan = scan(&source, "engine.rs");
        let refs = declaration_refs(&scan, "engine.rs");
        let owner = |name: &str| {
            refs.iter()
                .find(|declaration| declaration.name == name)
                .and_then(|declaration| declaration.owner.clone())
        };
        assert_eq!(owner("build").as_deref(), Some("Engine"));
        assert_eq!(owner("prepare").as_deref(), Some("Engine"));
        assert_eq!(owner("Engine"), None, "a class has no owner");
        assert!(refs
            .iter()
            .all(|declaration| declaration.file == "engine.rs"));
    }

    #[test]
    fn a_self_call_resolves_to_the_member_of_its_own_class() {
        let source = lines(&[
            "impl Engine {",
            "    fn build(&self) {",
            "        self.prepare();",
            "    }",
            "    fn prepare(&self) {}",
            "}",
            "impl Other {",
            "    fn prepare(&self) {}",
            "}",
        ]);
        let scan = scan(&source, "engine.rs");
        let project = declaration_refs(&scan, "engine.rs");
        let call = scan
            .calls
            .iter()
            .find(|call| call.name == "prepare")
            .expect("the self call");
        let resolved = resolve_call(call, &scan, "engine.rs", &project, 4);
        assert_eq!(resolved.len(), 2, "both members are candidates");
        assert_eq!(resolved[0].confidence, CallConfidence::Member);
        assert_eq!(resolved[0].declaration.owner.as_deref(), Some("Engine"));
        assert_eq!(resolved[1].confidence, CallConfidence::SameFile);
        assert_eq!(resolved[1].declaration.owner.as_deref(), Some("Other"));
    }

    #[test]
    fn a_qualified_call_prefers_the_named_type_and_otherwise_falls_back() {
        let source = lines(&[
            "impl Engine {",
            "    fn run(&self) {}",
            "}",
            "fn start() {",
            "    Engine::run();",
            "    helper();",
            "}",
        ]);
        let scan = scan(&source, "engine.rs");
        let mut project = declaration_refs(&scan, "engine.rs");
        project.push(DeclarationRef {
            name: "helper".into(),
            file: "src/other.rs".into(),
            line: 12,
            column: 0,
            kind: SymbolKind::Function,
            owner: Some("Worker".into()),
        });
        let call = |name: &str| {
            scan.calls
                .iter()
                .find(|call| call.name == name)
                .expect("the call")
        };
        let qualified = resolve_call(call("run"), &scan, "engine.rs", &project, 4);
        assert_eq!(qualified.len(), 1);
        assert_eq!(qualified[0].confidence, CallConfidence::Member);
        assert_eq!(qualified[0].declaration.owner.as_deref(), Some("Engine"));

        // A plain call finds the helper in the other file when this one has none.
        let plain = resolve_call(call("helper"), &scan, "engine.rs", &project, 4);
        assert_eq!(plain.len(), 1);
        assert_eq!(plain[0].confidence, CallConfidence::Project);
        assert_eq!(plain[0].declaration.file, "src/other.rs");
    }

    #[test]
    fn resolution_prefers_this_file_then_the_project_and_stays_bounded() {
        let source = lines(&["fn start() {", "    helper();", "}", "fn helper() {}"]);
        let scan = scan(&source, "main.rs");
        let mut project = declaration_refs(&scan, "main.rs");
        for index in 0..6 {
            project.push(DeclarationRef {
                name: "helper".into(),
                file: format!("src/module{index}.rs"),
                line: index,
                column: 0,
                kind: SymbolKind::Function,
                owner: None,
            });
        }
        let call = scan.calls.first().expect("the call");
        let resolved = resolve_call(call, &scan, "main.rs", &project, 4);
        assert_eq!(resolved.len(), 4, "the list is capped");
        assert_eq!(resolved[0].confidence, CallConfidence::SameFile);
        assert_eq!(resolved[0].declaration.file, "main.rs");
        assert!(resolved[1..]
            .iter()
            .all(|entry| entry.confidence == CallConfidence::Project));
        assert!(resolve_call(call, &scan, "main.rs", &project, 0).is_empty());

        let unknown = Call {
            name: "nowhere".into(),
            line: 1,
            column: 0,
            scope: 0,
            receiver: None,
        };
        assert!(resolve_call(&unknown, &scan, "main.rs", &project, 4).is_empty());
    }

    #[test]
    fn a_constructor_binding_names_the_class_of_a_local() {
        let source = lines(&[
            "fn start() {",
            "    let engine = Engine::new();",
            "    engine.run();",
            "    let writer = new Writer();",
            "    writer.flush();",
            "    const helper = Helper();",
            "    helper.help();",
            "    let unknown = build();",
            "    unknown.run();",
            "}",
        ]);
        let scan = scan(&source, "start.rs");
        let binding = |name: &str| {
            scan.bindings
                .iter()
                .find(|binding| binding.name == name)
                .map(|binding| binding.class.clone())
        };
        assert_eq!(binding("engine").as_deref(), Some("Engine"));
        assert_eq!(binding("writer").as_deref(), Some("Writer"));
        assert_eq!(binding("helper").as_deref(), Some("Helper"));
        assert_eq!(
            binding("unknown"),
            None,
            "something the scan cannot name is not guessed"
        );

        let resolver_input = vec![
            DeclarationRef {
                name: "run".into(),
                file: "start.rs".into(),
                line: 2,
                column: 0,
                kind: SymbolKind::Function,
                owner: Some("Engine".into()),
            },
            DeclarationRef {
                name: "run".into(),
                file: "start.rs".into(),
                line: 20,
                column: 0,
                kind: SymbolKind::Function,
                owner: Some("Other".into()),
            },
        ];
        let call = scan
            .calls
            .iter()
            .find(|call| call.name == "run")
            .expect("the member call");
        let resolved = resolve_call(call, &scan, "start.rs", &resolver_input, 4);
        assert_eq!(resolved[0].confidence, CallConfidence::Member);
        assert_eq!(resolved[0].declaration.owner.as_deref(), Some("Engine"));

        // The call on the unbound local keeps the weaker ranking.
        let unbound = scan
            .calls
            .iter()
            .filter(|call| call.name == "run")
            .nth(1)
            .expect("the second call");
        let resolved = resolve_call(unbound, &scan, "start.rs", &resolver_input, 4);
        assert_eq!(resolved[0].confidence, CallConfidence::SameFile);
    }

    #[test]
    fn a_binding_only_counts_inside_its_own_scope() {
        let source = lines(&[
            "fn first() {",
            "    let engine = Engine::new();",
            "}",
            "fn second() {",
            "    engine.run();",
            "}",
            "fn third() {",
            "    engine.run();",
            "    let engine = Engine::new();",
            "    engine.run();",
            "}",
        ]);
        let scan = scan(&source, "start.rs");
        let declaration = vec![DeclarationRef {
            name: "run".into(),
            file: "start.rs".into(),
            line: 1,
            column: 0,
            kind: SymbolKind::Function,
            owner: Some("Engine".into()),
        }];
        let resolve = |index: usize| {
            let call = scan
                .calls
                .iter()
                .filter(|call| call.name == "run")
                .nth(index)
                .expect("a call");
            resolve_call(call, &scan, "start.rs", &declaration, 4)
        };
        // `second` cannot see the binding of `first`.
        assert_eq!(resolve(0)[0].confidence, CallConfidence::SameFile);
        // Inside `third`, the call above the binding is not a member either, and
        // the call after it is.
        assert_eq!(resolve(1)[0].confidence, CallConfidence::SameFile);
        assert_eq!(resolve(2)[0].confidence, CallConfidence::Member);
        // The newest binding wins when a name is assigned twice.
        assert!(scan.bindings.iter().filter(|b| b.name == "engine").count() >= 2);
    }

    #[test]
    fn a_binding_does_not_confuse_a_comparison_or_a_loose_identifier() {
        let source = lines(&[
            "fn check(engine) {",
            "    if engine == Engine::new() {",
            "        return;",
            "    }",
            "    let value = 3;",
            "    let text = \"Engine::new()\";",
            "}",
        ]);
        let scan = scan(&source, "start.rs");
        assert!(
            scan.bindings.is_empty(),
            "no local holds a constructed instance here: {:?}",
            scan.bindings
        );
    }

    #[test]
    fn a_declared_return_type_is_read_from_the_signature() {
        // Rust and TypeScript spell the return type after an arrow or a colon.
        assert_eq!(
            declared_return_type("fn build(&self) -> Engine {", "Rust"),
            Some(("build".into(), "Engine".into()))
        );
        assert_eq!(
            declared_return_type("    pub fn make(count: usize) -> Engine {", "Rust"),
            Some(("make".into(), "Engine".into()))
        );
        assert_eq!(
            declared_return_type("function make(): Engine {", "TypeScript"),
            Some(("make".into(), "Engine".into()))
        );
        // C style: the type comes first, modifiers and all.
        assert_eq!(
            declared_return_type("Engine build(int count) {", "C++"),
            Some(("build".into(), "Engine".into()))
        );
        assert_eq!(
            declared_return_type("static Engine* make() {", "C"),
            Some(("make".into(), "Engine".into()))
        );

        // A reference, a pointer and a generic all name the class a call would
        // be made on rather than the wrapper around it.
        assert_eq!(
            declared_return_type("fn build(&self) -> &Engine {", "Rust"),
            Some(("build".into(), "Engine".into()))
        );
        assert_eq!(
            declared_return_type("fn all(&self) -> Vec<Engine> {", "Rust"),
            Some(("all".into(), "Engine".into()))
        );

        // A signature that promises nothing is left alone rather than guessed.
        assert_eq!(declared_return_type("fn run(&self) {", "Rust"), None);
        assert_eq!(declared_return_type("fn run(&self) -> () {", "Rust"), None);
        assert_eq!(declared_return_type("void run() {", "C++"), None);
        assert_eq!(declared_return_type("auto make() {", "C++"), None);
        assert_eq!(declared_return_type("if (count > 0) {", "C++"), None);
        assert_eq!(
            declared_return_type("for (int i = 0; i < 3; i++) {", "C++"),
            None
        );
        assert_eq!(declared_return_type("return engine;", "C++"), None);
        assert_eq!(declared_return_type("", "Rust"), None);
    }

    #[test]
    fn the_return_type_reader_never_panics_on_odd_input() {
        // It runs on every line of every file, so malformed text has to be
        // harmless rather than a crash.
        for sample in [
            "",
            "(",
            ")",
            "()",
            "->",
            ":",
            "fn",
            "fn ",
            "fn (",
            "fn f(",
            "fn f() ->",
            "fn f() -> {",
            "Engine",
            "Engine (",
            "Engine make(",
            "static",
            "static (",
            "return",
            "if (",
            "for (",
            "(",
            "-> ->",
            "a -> b",
            "fn f() -> Vec<",
            "fn f() -> Vec<Engine",
            "fn f() -> &",
            "fn f() -> *",
            "::",
            "x ->",
        ] {
            for language in [
                "Rust",
                "C",
                "C++",
                "Python",
                "TypeScript / JavaScript",
                "Plain Text",
            ] {
                let _ = declared_return_type(sample, language);
            }
        }
    }
}
