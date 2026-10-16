//! Project-level `.tcIDE` document.
//!
//! The document is deliberately plain text instead of a binary cache: TC-IDE,
//! an AI agent, and future tools can inspect it without starting the editor.
//! It stores the project identity, indexed files/symbols, relation edges and
//! the last graph view state. Unknown sections/keys are ignored for forward
//! compatibility.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

pub const FORMAT_VERSION: u32 = 1;
pub const FILE_EXTENSION: &str = "tcIDE";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectDocument {
    pub format: u32,
    pub name: String,
    pub root: String,
    pub language: String,
    pub language_version: String,
    pub environment: String,
    pub files: Vec<ProjectFile>,
    pub symbols: Vec<ProjectSymbol>,
    pub relations: Vec<ProjectRelation>,
    pub view: ProjectView,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectFile {
    pub path: String,
    pub language: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectSymbol {
    pub id: String,
    pub file: String,
    pub name: String,
    pub line: usize,
    pub role: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectRelation {
    pub from: String,
    pub to: String,
    pub kind: String,
    pub confidence: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectView {
    pub scope: String,
    pub focused_file: String,
    pub focused_symbol: String,
    pub folded: Vec<String>,
}

impl Default for ProjectView {
    fn default() -> Self {
        Self {
            scope: "file".into(),
            focused_file: String::new(),
            focused_symbol: String::new(),
            folded: Vec::new(),
        }
    }
}

impl Default for ProjectDocument {
    fn default() -> Self {
        Self {
            format: FORMAT_VERSION,
            ..Self::default_without_format()
        }
    }
}

impl ProjectDocument {
    fn default_without_format() -> Self {
        Self {
            format: FORMAT_VERSION,
            name: String::new(),
            root: ".".into(),
            language: String::new(),
            language_version: String::new(),
            environment: String::new(),
            files: Vec::new(),
            symbols: Vec::new(),
            relations: Vec::new(),
            view: ProjectView::default(),
        }
    }

    pub fn for_root(root: &Path) -> Self {
        Self {
            name: root
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("TC-IDE 项目")
                .to_string(),
            ..Self::default()
        }
    }
}

/// Return the conventional project document path, `<root>/<project>.tcIDE`.
pub fn document_path(root: &Path) -> PathBuf {
    let name = root
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("project");
    root.join(format!("{name}.{FILE_EXTENSION}"))
}

/// Find the first `.tcIDE` document in a project root, accepting any filename.
pub fn discover_path(root: &Path) -> Option<PathBuf> {
    let mut paths = fs::read_dir(root)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.eq_ignore_ascii_case(FILE_EXTENSION))
                    .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths.into_iter().next()
}

pub fn load(root: &Path) -> std::io::Result<ProjectDocument> {
    let path = discover_path(root).unwrap_or_else(|| document_path(root));
    let content = fs::read_to_string(path)?;
    Ok(parse(&content))
}

pub fn load_or_create(root: &Path) -> ProjectDocument {
    load(root).unwrap_or_else(|_| ProjectDocument::for_root(root))
}

pub fn save(root: &Path, document: &ProjectDocument) -> std::io::Result<PathBuf> {
    let path = document_path(root);
    fs::write(&path, serialize(document))?;
    Ok(path)
}

pub fn serialize(document: &ProjectDocument) -> String {
    let mut out = format!(
        "# TC-IDE project document — readable by TC-IDE and AI tools\nformat={}\n\n[project]\nname={}\nroot={}\nlanguage={}\nlanguage_version={}\nenvironment={}\n\n",
        document.format.max(1),
        encode(&document.name),
        encode(&document.root),
        encode(&document.language),
        encode(&document.language_version),
        encode(&document.environment),
    );
    out.push_str("[files]\n");
    for file in &document.files {
        out.push_str(&format!(
            "path={}\tlanguage={}\n",
            encode(&file.path),
            encode(&file.language)
        ));
    }
    out.push_str("\n[symbols]\n");
    for symbol in &document.symbols {
        out.push_str(&format!(
            "id={}\tfile={}\tname={}\tline={}\trole={}\n",
            encode(&symbol.id),
            encode(&symbol.file),
            encode(&symbol.name),
            symbol.line,
            encode(&symbol.role)
        ));
    }
    out.push_str("\n[relations]\n");
    for relation in &document.relations {
        out.push_str(&format!(
            "from={}\tto={}\tkind={}\tconfidence={}\n",
            encode(&relation.from),
            encode(&relation.to),
            encode(&relation.kind),
            encode(&relation.confidence)
        ));
    }
    out.push_str(&format!(
        "\n[view]\nscope={}\tfocused_file={}\tfocused_symbol={}\nfolded={}\n",
        encode(&document.view.scope),
        encode(&document.view.focused_file),
        encode(&document.view.focused_symbol),
        document
            .view
            .folded
            .iter()
            .map(|value| encode(value))
            .collect::<Vec<_>>()
            .join(",")
    ));
    out
}

pub fn parse(content: &str) -> ProjectDocument {
    let mut document = ProjectDocument::default();
    let mut section = String::new();
    for line in content.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line
            .strip_prefix('[')
            .and_then(|value| value.strip_suffix(']'))
        {
            section = name.to_string();
            continue;
        }
        if section == "files" {
            parse_file(line, &mut document.files);
            continue;
        }
        if section == "symbols" {
            parse_symbol(line, &mut document.symbols);
            continue;
        }
        if section == "relations" {
            parse_relation(line, &mut document.relations);
            continue;
        }
        if section == "view" {
            parse_view(line, &mut document.view);
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match (section.as_str(), key.trim()) {
            ("", "format") => document.format = value.trim().parse().unwrap_or(FORMAT_VERSION),
            ("project", "name") => document.name = decode(value),
            ("project", "root") => document.root = decode(value),
            ("project", "language") => document.language = decode(value),
            ("project", "language_version") => document.language_version = decode(value),
            ("project", "environment") => document.environment = decode(value),
            _ => {}
        }
    }
    document.format = document.format.max(1);
    document
}

fn parse_file(line: &str, files: &mut Vec<ProjectFile>) {
    let fields = fields(line);
    let path = fields.get("path").map(|v| decode(v)).unwrap_or_default();
    if path.is_empty() {
        return;
    }
    files.push(ProjectFile {
        path,
        language: fields
            .get("language")
            .map(|v| decode(v))
            .unwrap_or_default(),
    });
}

fn parse_symbol(line: &str, symbols: &mut Vec<ProjectSymbol>) {
    let fields = fields(line);
    let id = fields.get("id").map(|v| decode(v)).unwrap_or_default();
    if id.is_empty() {
        return;
    }
    symbols.push(ProjectSymbol {
        id,
        file: fields.get("file").map(|v| decode(v)).unwrap_or_default(),
        name: fields.get("name").map(|v| decode(v)).unwrap_or_default(),
        line: fields.get("line").and_then(|v| v.parse().ok()).unwrap_or(0),
        role: fields.get("role").map(|v| decode(v)).unwrap_or_default(),
    });
}

fn parse_relation(line: &str, relations: &mut Vec<ProjectRelation>) {
    let fields = fields(line);
    let from = fields.get("from").map(|v| decode(v)).unwrap_or_default();
    let to = fields.get("to").map(|v| decode(v)).unwrap_or_default();
    if from.is_empty() || to.is_empty() {
        return;
    }
    relations.push(ProjectRelation {
        from,
        to,
        kind: fields.get("kind").map(|v| decode(v)).unwrap_or_default(),
        confidence: fields
            .get("confidence")
            .map(|v| decode(v))
            .unwrap_or_default(),
    });
}

fn parse_view(line: &str, view: &mut ProjectView) {
    let fields = fields(line);
    if let Some(value) = fields.get("scope") {
        view.scope = decode(value);
    }
    if let Some(value) = fields.get("focused_file") {
        view.focused_file = decode(value);
    }
    if let Some(value) = fields.get("focused_symbol") {
        view.focused_symbol = decode(value);
    }
    if let Some(value) = fields.get("folded") {
        view.folded = value
            .split(',')
            .filter(|v| !v.is_empty())
            .map(decode)
            .collect();
    }
}

fn fields(line: &str) -> std::collections::BTreeMap<&str, &str> {
    line.split('\t')
        .filter_map(|field| field.split_once('='))
        .collect()
}

fn encode(value: &str) -> String {
    value
        .replace('%', "%25")
        .replace('\n', "%0A")
        .replace('\r', "%0D")
        .replace('\t', "%09")
}
fn decode(value: &str) -> String {
    let mut result = String::new();
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch == '%' {
            let code = chars.by_ref().take(2).collect::<String>();
            match code.as_str() {
                "25" => result.push('%'),
                "0A" => result.push('\n'),
                "0D" => result.push('\r'),
                "09" => result.push('\t'),
                _ => {
                    result.push('%');
                    result.push_str(&code);
                }
            }
        } else {
            result.push(ch);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_preserves_graph_and_utf8() {
        let mut document = ProjectDocument::for_root(Path::new("示例项目"));
        document.language = "C++".into();
        document.files.push(ProjectFile {
            path: "src/主文件.cpp".into(),
            language: "C++".into(),
        });
        document.symbols.push(ProjectSymbol {
            id: "symbol:src/主文件.cpp:3:main".into(),
            file: "src/主文件.cpp".into(),
            name: "main".into(),
            line: 3,
            role: "declaration".into(),
        });
        document.relations.push(ProjectRelation {
            from: "main".into(),
            to: "render_graph".into(),
            kind: "call".into(),
            confidence: "strong".into(),
        });
        document.view.folded.push("folder:src".into());
        assert_eq!(parse(&serialize(&document)), document);
    }

    #[test]
    fn unknown_content_is_ignored_and_format_is_safe() {
        let parsed = parse("format=0\n[project]\nname=demo\n[future]\nvalue=yes\n");
        assert_eq!(parsed.format, 1);
        assert_eq!(parsed.name, "demo");
    }
}
