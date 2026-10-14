//! Shared relation graph model.
//!
//! The graph is derived from the same project symbol index the editor already
//! builds, so scope selection, search and hierarchy are plain data
//! transformations that every native backend can render in its own way. Keeping
//! them here means `cargo test` on Linux verifies the graph the Windows backend
//! draws and hit tests.
#![allow(dead_code)] // The Linux and macOS backends adopt this model next.

use crate::editor_core::{self, ProjectSymbolOccurrence, SymbolKind, SymbolOccurrence, SymbolRole};

/// Safety valve so a huge project can never queue an unbounded node list.
pub const MAX_NODES: usize = 240;
/// Call sites listed under a single declaration.
const MAX_CALL_CHILDREN: usize = 3;
/// Members listed under a single class.
const MAX_MEMBER_CHILDREN: usize = 8;
/// Files listed under a folder or the project root.
const MAX_FILE_CHILDREN: usize = 12;

/// Range the graph covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphScope {
    Project,
    Folder,
    File,
    Symbol,
}

impl GraphScope {
    pub const ALL: [GraphScope; 4] = [
        GraphScope::Project,
        GraphScope::Folder,
        GraphScope::File,
        GraphScope::Symbol,
    ];

    pub fn label(self) -> &'static str {
        match self {
            GraphScope::Project => "project",
            GraphScope::Folder => "folder",
            GraphScope::File => "file",
            GraphScope::Symbol => "symbol",
        }
    }

    pub fn index(self) -> usize {
        match self {
            GraphScope::Project => 0,
            GraphScope::Folder => 1,
            GraphScope::File => 2,
            GraphScope::Symbol => 3,
        }
    }

    pub fn from_index(index: usize) -> Self {
        Self::ALL[index.min(Self::ALL.len() - 1)]
    }

    /// Next scope when the user cycles through them.
    pub fn next(self) -> Self {
        Self::from_index((self.index() + 1) % Self::ALL.len())
    }
}

/// Hierarchy level of a graph node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphLevel {
    Project,
    Folder,
    File,
    Class,
    Member,
    Symbol,
    Occurrence,
}

impl GraphLevel {
    pub fn depth(self) -> usize {
        match self {
            GraphLevel::Project => 0,
            GraphLevel::Folder | GraphLevel::File => 1,
            GraphLevel::Class | GraphLevel::Symbol => 2,
            GraphLevel::Member | GraphLevel::Occurrence => 3,
        }
    }

    /// Short prefix drawn in front of a node label.
    pub fn marker(self, role: Option<SymbolRole>) -> &'static str {
        match self {
            GraphLevel::Project => "project",
            GraphLevel::Folder => "folder",
            GraphLevel::File => "file",
            GraphLevel::Class => "class",
            GraphLevel::Member => "member",
            GraphLevel::Symbol => "symbol",
            GraphLevel::Occurrence => match role {
                Some(SymbolRole::Declaration) => "declare",
                _ => "call",
            },
        }
    }
}

/// Jump target of a node, relative to the project root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphTarget {
    pub file: String,
    pub line: usize,
    pub column: usize,
}

/// One drawn node of the relation graph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphNode {
    pub level: GraphLevel,
    pub label: String,
    pub detail: String,
    pub role: Option<SymbolRole>,
    pub target: Option<GraphTarget>,
    pub depth: usize,
}

impl GraphNode {
    fn new(level: GraphLevel, label: String, detail: String) -> Self {
        Self {
            level,
            label,
            detail,
            role: None,
            target: None,
            depth: level.depth(),
        }
    }

    fn with_role(mut self, role: SymbolRole) -> Self {
        self.role = Some(role);
        self
    }

    fn with_target(mut self, file: &str, line: usize, column: usize) -> Self {
        self.target = Some(GraphTarget {
            file: file.to_string(),
            line,
            column,
        });
        self
    }

    fn indented(mut self, parent: GraphLevel) -> Self {
        self.depth = parent.depth() + 1;
        self
    }

    fn nested(mut self, parent_depth: usize) -> Self {
        self.depth = parent_depth + 1;
        self
    }

    /// Marker and label the backends draw on the node card.
    pub fn caption(&self) -> String {
        format!("{}  {}", self.level.marker(self.role), self.label)
    }
}

/// Everything the graph needs from the editor and the project index.
pub struct GraphInputs<'a> {
    pub project_name: &'a str,
    pub current_file: &'a str,
    pub current_folder: &'a str,
    pub hovered: Option<&'a SymbolOccurrence>,
    pub file_lines: &'a [String],
    pub file_symbols: &'a [SymbolOccurrence],
    pub project_symbols: &'a [ProjectSymbolOccurrence],
}

/// Built graph for one frame: scope, search and the ordered node list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Graph {
    pub scope: GraphScope,
    pub search: String,
    pub nodes: Vec<GraphNode>,
    pub truncated: bool,
}

/// Scope used when the user has not pinned one.
pub fn auto_scope(
    hovered: Option<&SymbolOccurrence>,
    search: &str,
    pinned: Option<GraphScope>,
) -> GraphScope {
    if let Some(pinned) = pinned {
        return pinned;
    }
    if !search.trim().is_empty() {
        return GraphScope::Project;
    }
    if hovered.is_some() {
        return GraphScope::Symbol;
    }
    GraphScope::File
}

/// Folder part of a project relative path, empty at the project root.
pub fn folder_of(file: &str) -> &str {
    match file.rsplit_once('/') {
        Some((folder, _)) => folder,
        None => "",
    }
}

/// File name part of a project relative path.
pub fn file_name_of(file: &str) -> &str {
    match file.rsplit_once('/') {
        Some((_, name)) => name,
        None => file,
    }
}

pub fn build_graph(inputs: &GraphInputs<'_>, scope: GraphScope, search: &str) -> Graph {
    let query = search.trim().to_ascii_lowercase();
    let mut nodes = vec![GraphNode::new(
        GraphLevel::Project,
        inputs.project_name.to_string(),
        String::new(),
    )];
    let mut truncated = false;
    if !query.is_empty() {
        add_search_nodes(&mut nodes, inputs, &query, &mut truncated);
    } else {
        match scope {
            GraphScope::Project => add_project_nodes(&mut nodes, inputs, &mut truncated),
            GraphScope::Folder => add_folder_nodes(&mut nodes, inputs, &mut truncated),
            GraphScope::File => add_file_nodes(&mut nodes, inputs, &mut truncated),
            GraphScope::Symbol => add_symbol_nodes(&mut nodes, inputs, &mut truncated),
        }
    }
    if nodes.len() > MAX_NODES {
        nodes.truncate(MAX_NODES);
        truncated = true;
    }
    Graph {
        scope,
        search: search.to_string(),
        nodes,
        truncated,
    }
}

fn push(nodes: &mut Vec<GraphNode>, node: GraphNode, truncated: &mut bool) {
    if nodes.len() >= MAX_NODES {
        *truncated = true;
        return;
    }
    nodes.push(node);
}

fn add_search_nodes(
    nodes: &mut Vec<GraphNode>,
    inputs: &GraphInputs<'_>,
    query: &str,
    truncated: &mut bool,
) {
    let mut matches = inputs
        .project_symbols
        .iter()
        .filter(|entry| {
            entry.symbol.name.to_ascii_lowercase().contains(query)
                || entry.file.to_ascii_lowercase().contains(query)
        })
        .map(|entry| (entry.file.as_str(), entry.symbol.clone()))
        .collect::<Vec<_>>();
    if matches.is_empty() {
        matches = inputs
            .file_symbols
            .iter()
            .filter(|symbol| symbol.name.to_ascii_lowercase().contains(query))
            .map(|symbol| (inputs.current_file, symbol.clone()))
            .collect();
    }
    matches.sort_by(|left, right| {
        let left_declaration = left.1.role == SymbolRole::Declaration;
        let right_declaration = right.1.role == SymbolRole::Declaration;
        right_declaration
            .cmp(&left_declaration)
            .then_with(|| left.0.cmp(right.0))
            .then_with(|| left.1.line.cmp(&right.1.line))
            .then_with(|| left.1.name.cmp(&right.1.name))
    });
    matches.dedup_by(|left, right| {
        left.0 == right.0 && left.1.line == right.1.line && left.1.name == right.1.name
    });
    let found = matches.len();
    push(
        nodes,
        GraphNode::new(
            GraphLevel::Symbol,
            format!("results for \"{query}\""),
            format!("{found} matches"),
        ),
        truncated,
    );
    for (file, symbol) in matches {
        push(
            nodes,
            GraphNode::new(
                GraphLevel::Occurrence,
                format!("{}()", symbol.name),
                format!("{}:{}", file, symbol.line + 1),
            )
            .with_role(symbol.role)
            .with_target(file, symbol.line, symbol.start)
            .indented(GraphLevel::Symbol),
            truncated,
        );
    }
}

fn add_symbol_nodes(nodes: &mut Vec<GraphNode>, inputs: &GraphInputs<'_>, truncated: &mut bool) {
    let Some(hovered) = inputs.hovered else {
        add_file_nodes(nodes, inputs, truncated);
        return;
    };
    push(
        nodes,
        GraphNode::new(
            GraphLevel::File,
            inputs.current_file.to_string(),
            "current file".to_string(),
        )
        .with_target(inputs.current_file, hovered.line, hovered.start),
        truncated,
    );
    push(
        nodes,
        GraphNode::new(
            GraphLevel::Symbol,
            format!("{}()", hovered.name),
            format!("{}:{}", inputs.current_file, hovered.line + 1),
        )
        .with_target(inputs.current_file, hovered.line, hovered.start),
        truncated,
    );
    let mut occurrences = inputs
        .project_symbols
        .iter()
        .filter(|entry| entry.symbol.name == hovered.name)
        .collect::<Vec<_>>();
    occurrences.sort_by(|left, right| {
        let left_declaration = left.symbol.role == SymbolRole::Declaration;
        let right_declaration = right.symbol.role == SymbolRole::Declaration;
        right_declaration
            .cmp(&left_declaration)
            .then_with(|| left.file.cmp(&right.file))
            .then_with(|| left.symbol.line.cmp(&right.symbol.line))
    });
    if occurrences.is_empty() {
        push(
            nodes,
            GraphNode::new(
                GraphLevel::Occurrence,
                format!("{}()", hovered.name),
                format!("{}:{}", inputs.current_file, hovered.line + 1),
            )
            .with_role(hovered.role)
            .with_target(inputs.current_file, hovered.line, hovered.start)
            .indented(GraphLevel::Symbol),
            truncated,
        );
        return;
    }
    for entry in occurrences {
        let detail = if entry.file == inputs.current_file {
            format!("this file :{}", entry.symbol.line + 1)
        } else {
            format!("{}:{}", entry.file, entry.symbol.line + 1)
        };
        push(
            nodes,
            GraphNode::new(
                GraphLevel::Occurrence,
                format!("{}()", entry.symbol.name),
                detail,
            )
            .with_role(entry.symbol.role)
            .with_target(&entry.file, entry.symbol.line, entry.symbol.start)
            .indented(GraphLevel::Symbol),
            truncated,
        );
    }
}

/// Line a brace or indentation block opened on `start` ends on.
///
/// Brace languages close on the matching `}`; indentation languages close
/// before the first following line that returns to the starting indentation.
pub fn block_end(lines: &[String], start: usize) -> usize {
    let Some(opening) = lines.get(start) else {
        return start;
    };
    let base = indentation(opening);
    let braced = opening.contains('{');
    let mut depth = 0i32;
    for (index, line) in lines.iter().enumerate().skip(start) {
        let trimmed = line.trim();
        depth += trimmed.matches('{').count() as i32 - trimmed.matches('}').count() as i32;
        if index == start {
            continue;
        }
        if braced && depth <= 0 {
            return index.saturating_sub(1);
        }
        if !trimmed.is_empty() && indentation(line) <= base && depth <= 0 {
            return index.saturating_sub(1);
        }
    }
    lines.len().saturating_sub(1)
}

fn indentation(line: &str) -> usize {
    line.chars()
        .take_while(|value| *value == ' ' || *value == '\t')
        .count()
}

fn add_file_nodes(nodes: &mut Vec<GraphNode>, inputs: &GraphInputs<'_>, truncated: &mut bool) {
    push(
        nodes,
        GraphNode::new(
            GraphLevel::File,
            inputs.current_file.to_string(),
            inputs.current_folder.to_string(),
        )
        .with_target(inputs.current_file, 0, 0),
        truncated,
    );
    let mut declarations = declaration_kinds(inputs);
    if declarations.is_empty() {
        push(
            nodes,
            GraphNode::new(
                GraphLevel::Symbol,
                "no declarations yet".to_string(),
                "type a function or class".to_string(),
            )
            .indented(GraphLevel::File),
            truncated,
        );
        return;
    }
    declarations.sort_by_key(|(line, _, _)| *line);
    let class_blocks = declarations
        .iter()
        .filter(|(_, _, kind)| *kind == SymbolKind::Class)
        .map(|(line, _, _)| (*line, block_end(inputs.file_lines, *line)))
        .collect::<Vec<_>>();
    let mut consumed = vec![false; declarations.len()];
    for (index, (line, name, kind)) in declarations.iter().enumerate() {
        if consumed[index] {
            continue;
        }
        if *kind == SymbolKind::Class {
            push(
                nodes,
                GraphNode::new(
                    GraphLevel::Class,
                    format!("{}()", name),
                    format!(":{}", line + 1),
                )
                .with_target(inputs.current_file, *line, 0)
                .indented(GraphLevel::File),
                truncated,
            );
            let class_depth = GraphLevel::File.depth() + 1;
            let end = class_blocks
                .iter()
                .find(|(start, _)| start == line)
                .map(|(_, end)| *end)
                .unwrap_or(*line);
            for (member_index, (member_line, member, member_kind)) in
                declarations.iter().enumerate().skip(index + 1)
            {
                if *member_line > end || *member_kind == SymbolKind::Class {
                    break;
                }
                consumed[member_index] = true;
                if nodes.len() >= MAX_NODES {
                    *truncated = true;
                    break;
                }
                push(
                    nodes,
                    GraphNode::new(
                        GraphLevel::Member,
                        format!("{}()", member),
                        format!(":{}", member_line + 1),
                    )
                    .with_target(inputs.current_file, *member_line, 0)
                    .nested(class_depth),
                    truncated,
                );
            }
            continue;
        }
        if class_blocks
            .iter()
            .any(|(start, end)| line > start && line <= end)
        {
            continue;
        }
        push(
            nodes,
            GraphNode::new(
                GraphLevel::Member,
                format!("{}()", name),
                format!(":{}", line + 1),
            )
            .with_target(inputs.current_file, *line, 0)
            .indented(GraphLevel::File),
            truncated,
        );
        let declaration_depth = GraphLevel::File.depth() + 1;
        let calls = inputs
            .file_symbols
            .iter()
            .filter(|symbol| symbol.name == *name && symbol.role == SymbolRole::Reference)
            .take(MAX_CALL_CHILDREN);
        for call in calls {
            push(
                nodes,
                GraphNode::new(
                    GraphLevel::Occurrence,
                    format!("{}()", call.name),
                    format!(":{}", call.line + 1),
                )
                .with_role(SymbolRole::Reference)
                .with_target(inputs.current_file, call.line, call.start)
                .nested(declaration_depth),
                truncated,
            );
        }
    }
}

fn add_folder_nodes(nodes: &mut Vec<GraphNode>, inputs: &GraphInputs<'_>, truncated: &mut bool) {
    let folder = if inputs.current_folder.is_empty() {
        "project root"
    } else {
        inputs.current_folder
    };
    push(
        nodes,
        GraphNode::new(GraphLevel::Folder, folder.to_string(), String::new()),
        truncated,
    );
    let mut files = project_files(inputs);
    files.retain(|(file, _)| folder_of(file) == inputs.current_folder);
    if files.is_empty() {
        files.push((
            inputs.current_file.to_string(),
            declaration_kinds(inputs).len(),
        ));
    }
    files.sort_by(|left, right| left.0.cmp(&right.0));
    for (file, count) in files.into_iter().take(MAX_FILE_CHILDREN) {
        push(
            nodes,
            GraphNode::new(
                GraphLevel::File,
                file_name_of(&file).to_string(),
                format!("{count} symbols"),
            )
            .with_target(&file, 0, 0)
            .indented(GraphLevel::Folder),
            truncated,
        );
    }
}

fn add_project_nodes(nodes: &mut Vec<GraphNode>, inputs: &GraphInputs<'_>, truncated: &mut bool) {
    let files = project_files(inputs);
    let mut folders = Vec::<String>::new();
    for (file, _) in &files {
        let folder = folder_of(file);
        let folder = if folder.is_empty() {
            "project root"
        } else {
            folder
        };
        if !folders.iter().any(|known| known == folder) {
            folders.push(folder.to_string());
        }
    }
    if folders.is_empty() {
        folders.push("project root".to_string());
    }
    for folder in folders.into_iter().take(MAX_FILE_CHILDREN) {
        let wanted = if folder == "project root" {
            ""
        } else {
            folder.as_str()
        };
        let in_folder = files
            .iter()
            .filter(|(file, _)| folder_of(file) == wanted)
            .collect::<Vec<_>>();
        push(
            nodes,
            GraphNode::new(
                GraphLevel::Folder,
                folder.clone(),
                format!("{} files", in_folder.len()),
            ),
            truncated,
        );
        for (file, count) in in_folder.into_iter().take(MAX_FILE_CHILDREN) {
            let is_current = file == inputs.current_file;
            push(
                nodes,
                GraphNode::new(
                    GraphLevel::File,
                    file_name_of(file).to_string(),
                    if is_current {
                        format!("{count} symbols · current")
                    } else {
                        format!("{count} symbols")
                    },
                )
                .with_target(file, 0, 0)
                .indented(GraphLevel::Folder),
                truncated,
            );
        }
    }
}

fn project_files(inputs: &GraphInputs<'_>) -> Vec<(String, usize)> {
    let mut files = Vec::<(String, usize)>::new();
    for entry in inputs.project_symbols {
        if let Some(existing) = files.iter_mut().find(|(file, _)| *file == entry.file) {
            if entry.symbol.role == SymbolRole::Declaration {
                existing.1 += 1;
            }
            continue;
        }
        files.push((
            entry.file.clone(),
            usize::from(entry.symbol.role == SymbolRole::Declaration),
        ));
    }
    if !inputs.current_file.is_empty() && !files.iter().any(|(file, _)| file == inputs.current_file)
    {
        files.push((inputs.current_file.to_string(), 0));
    }
    files.sort_by(|left, right| left.0.cmp(&right.0));
    files
}

/// Declarations of the current file as `(line, name, kind)`.
fn declaration_kinds(inputs: &GraphInputs<'_>) -> Vec<(usize, String, SymbolKind)> {
    let mut declarations = Vec::new();
    for symbol in inputs.file_symbols {
        if symbol.role != SymbolRole::Declaration {
            continue;
        }
        if declarations
            .iter()
            .any(|(line, name, _): &(usize, String, SymbolKind)| {
                *line == symbol.line && *name == symbol.name
            })
        {
            continue;
        }
        let kind = inputs
            .file_lines
            .get(symbol.line)
            .map(|line| editor_core::declaration_kind(line, inputs.current_file))
            .unwrap_or(SymbolKind::Other);
        declarations.push((symbol.line, symbol.name.clone(), kind));
    }
    declarations.sort_by_key(|(line, _, _)| *line);
    declarations
}

impl Graph {
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn max_scroll(&self, visible_rows: usize) -> usize {
        self.nodes.len().saturating_sub(visible_rows.max(1))
    }

    pub fn clamped_scroll(&self, scroll: usize, visible_rows: usize) -> usize {
        scroll.min(self.max_scroll(visible_rows))
    }

    /// Nodes shown at a scroll offset, clipped to the viewport.
    pub fn visible(&self, scroll: usize, visible_rows: usize) -> &[GraphNode] {
        let first = self.clamped_scroll(scroll, visible_rows);
        let last = first
            .saturating_add(visible_rows.max(1))
            .min(self.nodes.len());
        if first >= last {
            &[]
        } else {
            &self.nodes[first..last]
        }
    }

    /// Backends label the panel with this summary.
    pub fn summary(&self) -> String {
        let scope = if self.search.trim().is_empty() {
            self.scope.label().to_string()
        } else {
            format!("search \"{}\"", self.search.trim())
        };
        if self.truncated {
            format!("range {scope} · {} nodes · truncated", self.nodes.len())
        } else {
            format!("range {scope} · {} nodes", self.nodes.len())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn occurrence(name: &str, line: usize, role: SymbolRole) -> SymbolOccurrence {
        SymbolOccurrence {
            name: name.to_string(),
            line,
            start: 4,
            end: 4 + name.len(),
            role,
        }
    }

    fn project_entry(
        file: &str,
        name: &str,
        line: usize,
        role: SymbolRole,
    ) -> ProjectSymbolOccurrence {
        ProjectSymbolOccurrence {
            file: file.to_string(),
            symbol: occurrence(name, line, role),
        }
    }

    fn sample_lines() -> Vec<String> {
        vec![
            "class Engine {".into(),
            "    pub fn start(&self) {".into(),
            "        run();".into(),
            "    }".into(),
            "}".into(),
            "pub fn run() {".into(),
            "    start();".into(),
            "}".into(),
            "const LIMIT: usize = 4;".into(),
        ]
    }

    fn inputs_current_file<'a>(
        lines: &'a [String],
        symbols: &'a [SymbolOccurrence],
        project: &'a [ProjectSymbolOccurrence],
        hovered: Option<&'a SymbolOccurrence>,
    ) -> GraphInputs<'a> {
        GraphInputs {
            project_name: "demo",
            current_file: "src/engine.rs",
            current_folder: "src",
            hovered,
            file_lines: lines,
            file_symbols: symbols,
            project_symbols: project,
        }
    }

    #[test]
    fn scope_prefers_hover_then_current_file() {
        let hovered = occurrence("run", 5, SymbolRole::Declaration);
        assert_eq!(auto_scope(Some(&hovered), "", None), GraphScope::Symbol);
        assert_eq!(auto_scope(None, "", None), GraphScope::File);
        assert_eq!(auto_scope(None, "run", None), GraphScope::Project);
        assert_eq!(
            auto_scope(Some(&hovered), "run", Some(GraphScope::Folder)),
            GraphScope::Folder
        );
        assert_eq!(GraphScope::File.next(), GraphScope::Symbol);
        assert_eq!(GraphScope::Symbol.next(), GraphScope::Project);
    }

    #[test]
    fn file_scope_nests_class_members_and_function_calls() {
        let lines = sample_lines();
        let symbols = editor_core::symbol_occurrences(&lines, "src/engine.rs");
        let project = vec![
            project_entry("src/engine.rs", "Engine", 0, SymbolRole::Declaration),
            project_entry("src/engine.rs", "start", 1, SymbolRole::Declaration),
            project_entry("src/engine.rs", "run", 5, SymbolRole::Declaration),
        ];
        let inputs = inputs_current_file(&lines, &symbols, &project, None);
        let graph = build_graph(&inputs, GraphScope::File, "");
        let captions = graph
            .nodes
            .iter()
            .map(|node| (node.depth, node.caption()))
            .collect::<Vec<_>>();
        assert_eq!(captions[0], (0, "project  demo".into()));
        assert_eq!(captions[1], (1, "file  src/engine.rs".into()));
        assert!(captions.contains(&(2, "class  Engine()".into())));
        assert!(captions.contains(&(3, "member  start()".into())));
        assert!(captions.contains(&(2, "member  run()".into())));
        assert!(
            captions.contains(&(3, "call  run()".into())),
            "{captions:?}"
        );
        assert!(!graph.truncated);
    }

    #[test]
    fn variable_declarations_become_member_nodes() {
        let lines = vec!["const LIMIT: usize = 4;".into()];
        let symbols = editor_core::symbol_occurrences(&lines, "src/engine.rs");
        let project = vec![project_entry(
            "src/engine.rs",
            "LIMIT",
            0,
            SymbolRole::Declaration,
        )];
        let inputs = inputs_current_file(&lines, &symbols, &project, None);
        let graph = build_graph(&inputs, GraphScope::File, "");
        assert!(graph
            .nodes
            .iter()
            .any(|node| node.caption() == "member  LIMIT()"));
    }

    #[test]
    fn file_scope_reports_an_empty_file() {
        let lines = vec!["// nothing yet".into()];
        let inputs = inputs_current_file(&lines, &[], &[], None);
        let graph = build_graph(&inputs, GraphScope::File, "");
        assert_eq!(graph.len(), 3);
        assert!(graph.nodes[2].label.contains("no declarations"));
        assert_eq!(graph.summary(), "range file · 3 nodes");
    }

    #[test]
    fn symbol_scope_lists_declarations_before_calls() {
        let lines = sample_lines();
        let symbols = editor_core::symbol_occurrences(&lines, "src/engine.rs");
        let hovered = occurrence("run", 5, SymbolRole::Declaration);
        let project = vec![
            project_entry("src/lib.rs", "run", 2, SymbolRole::Reference),
            project_entry("src/engine.rs", "run", 5, SymbolRole::Declaration),
            project_entry("src/main.rs", "run", 9, SymbolRole::Reference),
        ];
        let inputs = inputs_current_file(&lines, &symbols, &project, Some(&hovered));
        let graph = build_graph(&inputs, GraphScope::Symbol, "");
        assert_eq!(graph.nodes[2].caption(), "symbol  run()");
        let roles = graph.nodes[3..]
            .iter()
            .map(|node| node.caption())
            .collect::<Vec<_>>();
        assert_eq!(roles[0], "declare  run()");
        assert_eq!(roles[1], "call  run()");
        assert_eq!(roles[2], "call  run()");
        assert_eq!(
            graph.nodes[3]
                .target
                .as_ref()
                .map(|target| target.file.as_str()),
            Some("src/engine.rs")
        );
    }

    #[test]
    fn search_matches_names_and_paths() {
        let lines = sample_lines();
        let symbols = editor_core::symbol_occurrences(&lines, "src/engine.rs");
        let project = vec![
            project_entry("src/engine.rs", "run", 5, SymbolRole::Declaration),
            project_entry("src/network.rs", "run_client", 3, SymbolRole::Declaration),
            project_entry("src/network.rs", "run_client", 20, SymbolRole::Reference),
        ];
        let inputs = inputs_current_file(&lines, &symbols, &project, None);
        let graph = build_graph(&inputs, GraphScope::File, "network");
        assert_eq!(graph.nodes[1].label, "results for \"network\"");
        assert_eq!(graph.nodes[1].detail, "2 matches");
        assert!(graph
            .nodes
            .iter()
            .all(|node| node.caption() != "member  run()"));

        let by_name = build_graph(&inputs, GraphScope::File, "RUN");
        assert!(by_name
            .nodes
            .iter()
            .any(|node| node.caption() == "declare  run()"));
        assert_eq!(by_name.summary(), "range search \"RUN\" · 5 nodes");
    }

    #[test]
    fn folder_and_project_scopes_group_files() {
        let lines = sample_lines();
        let symbols = editor_core::symbol_occurrences(&lines, "src/engine.rs");
        let project = vec![
            project_entry("src/engine.rs", "run", 5, SymbolRole::Declaration),
            project_entry("src/network.rs", "connect", 1, SymbolRole::Declaration),
            project_entry("src/network.rs", "connect", 8, SymbolRole::Reference),
            project_entry("README.md", "Title", 0, SymbolRole::Declaration),
        ];
        let inputs = inputs_current_file(&lines, &symbols, &project, None);

        let folder = build_graph(&inputs, GraphScope::Folder, "");
        assert_eq!(folder.nodes[1].caption(), "folder  src");
        let files = folder
            .nodes
            .iter()
            .filter(|node| node.level == GraphLevel::File)
            .map(|node| node.label.clone())
            .collect::<Vec<_>>();
        assert_eq!(files, ["engine.rs", "network.rs"]);
        assert!(folder
            .nodes
            .iter()
            .any(|node| node.detail == "1 symbols" && node.label == "network.rs"));

        let project_scope = build_graph(&inputs, GraphScope::Project, "");
        let folders = project_scope
            .nodes
            .iter()
            .filter(|node| node.level == GraphLevel::Folder)
            .map(|node| node.label.clone())
            .collect::<Vec<_>>();
        assert_eq!(folders, ["project root", "src"]);
        assert!(project_scope
            .nodes
            .iter()
            .any(|node| node.label == "README.md" && node.depth == GraphLevel::File.depth() + 1));
    }

    #[test]
    fn viewport_clips_and_clamps_scrolling() {
        let lines = sample_lines();
        let symbols = editor_core::symbol_occurrences(&lines, "src/engine.rs");
        let inputs = inputs_current_file(&lines, &symbols, &[], None);
        let graph = build_graph(&inputs, GraphScope::File, "");
        assert!(graph.len() > 3);
        assert_eq!(graph.max_scroll(3), graph.len() - 3);
        assert_eq!(graph.clamped_scroll(99, 3), graph.len() - 3);
        assert_eq!(graph.visible(0, 2).len(), 2);
        assert_eq!(graph.visible(99, 2).last(), graph.nodes.last());
        assert_eq!(graph.visible(graph.len(), 2).len(), 2);
        assert_eq!(graph.visible(0, graph.len() + 4).len(), graph.len());
        assert_eq!(graph.max_scroll(0), graph.len() - 1);
    }

    #[test]
    fn node_count_is_bounded_for_huge_indexes() {
        let lines = vec!["pub fn run() {}".into()];
        let symbols = editor_core::symbol_occurrences(&lines, "src/engine.rs");
        let project = (0..(MAX_NODES * 2))
            .map(|index| {
                project_entry(
                    &format!("src/file{}.rs", index % 40),
                    &format!("symbol{index}"),
                    index,
                    SymbolRole::Reference,
                )
            })
            .collect::<Vec<_>>();
        let inputs = inputs_current_file(&lines, &symbols, &project, None);
        let graph = build_graph(&inputs, GraphScope::File, "symbol");
        assert!(graph.len() <= MAX_NODES);
        assert!(graph.truncated);
        assert!(graph.summary().contains("truncated"));
        let folders = build_graph(&inputs, GraphScope::Project, "");
        assert!(
            folders
                .nodes
                .iter()
                .filter(|node| node.level == GraphLevel::File)
                .count()
                <= MAX_FILE_CHILDREN
        );
    }

    #[test]
    fn block_ranges_follow_braces_and_indentation() {
        let braced = vec![
            "class A {".to_string(),
            "  fn x() {".to_string(),
            "  }".to_string(),
            "}".to_string(),
            "fn y() {}".to_string(),
        ];
        assert_eq!(block_end(&braced, 0), 2);
        assert_eq!(block_end(&braced, 4), 4);
        let python = vec![
            "class A:".to_string(),
            "    def x(self):".to_string(),
            "        pass".to_string(),
            "def y():".to_string(),
        ];
        assert_eq!(block_end(&python, 0), 2);
        assert_eq!(block_end(&python, 3), 3);
        assert_eq!(block_end(&[], 0), 0);
    }

    #[test]
    fn path_helpers_handle_root_and_nested_files() {
        assert_eq!(folder_of("main.rs"), "");
        assert_eq!(folder_of("src/app/main.rs"), "src/app");
        assert_eq!(file_name_of("src/app/main.rs"), "main.rs");
    }
}
