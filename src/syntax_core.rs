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

/// A call shaped identifier occurrence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Call {
    pub name: String,
    pub line: usize,
    pub column: usize,
    /// Index of the innermost scope the call appears in.
    pub scope: usize,
}

/// Everything the scan learned about one file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileScan {
    pub scopes: Vec<Scope>,
    pub declarations: Vec<Declaration>,
    pub calls: Vec<Call>,
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
                    let kind = if first_open && opened_function.is_some() {
                        if symbol_kind == Some(SymbolKind::Class) {
                            ScopeKind::Class
                        } else {
                            ScopeKind::Function
                        }
                    } else {
                        ScopeKind::Block
                    };
                    let parent = *stack.last().unwrap_or(&0);
                    scopes.push(Scope {
                        kind,
                        name: if first_open {
                            opened_function.clone()
                        } else {
                            None
                        },
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

        // Calls: an identifier followed by an opening parenthesis that is not
        // the declaration on this line and not a control keyword.
        let inner_scope = *stack.last().unwrap_or(&0);
        for (start, end) in call_positions(&masked) {
            let name = &masked[start..end];
            if declaration.as_deref() == Some(name) {
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
    }
}

/// Byte ranges of `name(` patterns in masked text.
fn call_positions(masked: &str) -> Vec<(usize, usize)> {
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
            // Skip a member call like `self.run(`, which belongs to the object.
            let before = &masked[..start];
            let qualified = before.trim_end().ends_with('.') || before.trim_end().ends_with("::");
            if !qualified {
                positions.push((start, end));
            }
        }
    }
    positions
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
    fn calls_ignore_comments_strings_and_member_prefixes() {
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
        assert_eq!(scan.called_names(function), ["helper"]);
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
}
