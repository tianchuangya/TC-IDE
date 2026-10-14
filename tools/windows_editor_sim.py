#!/usr/bin/env python3
"""Run the Windows editor's editing logic on Linux.

Windows-only code cannot be executed on a Linux workstation, which makes an
editor regression invisible until the Windows CI job runs. This script extracts
the editing methods from the `src/windows_app` module into a small scratch crate that
links the real `text_buffer` and `editor_core` modules, drives a set of editing
sequences and reports any mismatch.

Usage:

    python3 tools/windows_editor_sim.py [--work /tmp/editsim]

It is a development aid, not a test: the real tests live in the Windows test
module and run in CI. It exists because it found a real bug (a cut that never
closed its edit group, so undo could not revert it) that no Linux test covered.
"""

import argparse
import os
import re
import shutil
import subprocess
import sys

METHODS = [
    "update_tree_rows", "tree_row_position",
    "clamp_caret", "position", "snapshot", "restore", "sync_lines", "record_undo",
    "finish_edit", "selection_range", "selected_text", "delete_selection_raw",
    "insert_text", "newline", "newline_raw", "backspace", "delete_forward",
    "cut_selection", "accept_completion", "completions", "comment_edit_bounds",
    "clamp_comment_caret", "byte_at", "move_left", "move_right", "undo", "redo",
    "set_caret", "reveal_caret",
]

HARNESS = '''#[path = "{buffer}"]
mod text_buffer;
#[path = "{editor_core}"]
mod editor_core;
#[path = "{motion_core}"]
mod motion_core;

use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct TextPosition {{ line: usize, column: usize }}

#[derive(Clone)]
struct EditorSnapshot {{ caret: TextPosition, selection_anchor: Option<TextPosition> }}

struct App {{
    lines: Vec<String>,
    buffer: text_buffer::TextBuffer,
    caret_line: usize,
    caret_column: usize,
    selection_anchor: Option<TextPosition>,
    undo_stack: Vec<EditorSnapshot>,
    redo_stack: Vec<EditorSnapshot>,
    last_edit: Option<Instant>,
    status: String,
    lsp_dirty: bool,
    file_name: String,
    lsp_completions: Vec<String>,
    editing_comment: Option<usize>,
    viewport: text_buffer::Viewport,
    workspace_search: String,
    project_entries: Vec<String>,
    project_entry_paths: Vec<std::path::PathBuf>,
    tree_row_shift: motion_core::RowShift,
    tree_row_positions: Vec<f32>,
}}

impl App {{
    fn new() -> Self {{
        Self {{
            lines: vec![String::new()],
            buffer: text_buffer::TextBuffer::default(),
            caret_line: 0,
            caret_column: 0,
            selection_anchor: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            last_edit: None,
            status: String::new(),
            lsp_dirty: false,
            file_name: "main.rs".into(),
            lsp_completions: Vec::new(),
            editing_comment: None,
            viewport: text_buffer::Viewport::new(24, 96),
            workspace_search: String::new(),
            project_entries: Vec::new(),
            project_entry_paths: Vec::new(),
            tree_row_shift: motion_core::RowShift::new(),
            tree_row_positions: Vec::new(),
        }}
    }}

    fn mark_active_dirty(&mut self) {{}}
    fn notify_language_server(&mut self) {{}}
    fn update_active_index_entry(&mut self) {{}}
    fn ensure_language_server(&mut self) {{}}
    fn code_cell_width(&self) -> f32 {{ 8.9 }}

    fn seed(&mut self, lines: &[&str]) {{
        self.lines = lines.iter().map(|value| (*value).to_string()).collect();
        self.buffer = text_buffer::TextBuffer::from_lines(&self.lines);
        self.caret_line = 0;
        self.caret_column = 0;
        self.selection_anchor = None;
        self.undo_stack.clear();
        self.redo_stack.clear();
    }}

{methods}
}}

fn check(label: &str, actual: &[String], expected: &[&str], failures: &mut Vec<String>) {{
    let expected = expected.iter().map(|value| (*value).to_string()).collect::<Vec<_>>();
    if actual != expected.as_slice() {{
        failures.push(format!("{{label}}: got {{actual:?}} expected {{expected:?}}"));
    }}
}}

fn main() {{
    let mut failures = Vec::new();

    let mut app = App::new();
    app.seed(&["one", "two"]);
    app.caret_line = 0;
    app.caret_column = 3;
    app.insert_text("!");
    check("after insert", &app.lines, &["one!", "two"], &mut failures);
    app.newline();
    app.insert_text("inserted");
    check("after newline and text", &app.lines, &["one!", "inserted", "two"], &mut failures);
    app.backspace();
    check("after backspace", &app.lines, &["one!", "inserte", "two"], &mut failures);
    app.undo();
    check("undo 1", &app.lines, &["one!", "inserted", "two"], &mut failures);
    app.undo();
    check("undo 2", &app.lines, &["one!", "", "two"], &mut failures);
    app.undo();
    check("undo 3", &app.lines, &["one!", "two"], &mut failures);
    app.undo();
    check("undo 4", &app.lines, &["one", "two"], &mut failures);
    app.redo();
    check("redo", &app.lines, &["one!", "two"], &mut failures);
    app.caret_line = 1;
    app.caret_column = 3;
    app.insert_text(" 你好");
    check("unicode paste", &app.lines, &["one!", "two 你好"], &mut failures);
    app.undo();
    check("unicode undo", &app.lines, &["one!", "two"], &mut failures);

    let mut app = App::new();
    app.seed(&["alpha", "beta"]);
    app.selection_anchor = Some(TextPosition {{ line: 0, column: 2 }});
    app.caret_line = 1;
    app.caret_column = 2;
    let cut = app.cut_selection();
    check("cut text", &[cut.clone().unwrap_or_default()], &["pha\\nbe"], &mut failures);
    check("after cut", &app.lines, &["alta"], &mut failures);
    app.undo();
    check("cut undo", &app.lines, &["alpha", "beta"], &mut failures);

    let mut app = App::new();
    app.seed(&["你好 world"]);
    app.caret_column = 2;
    app.insert_text("真的");
    check("unicode insert", &app.lines, &["你好真的 world"], &mut failures);
    app.backspace();
    check("unicode backspace", &app.lines, &["你好真 world"], &mut failures);

    let mut app = App::new();
    app.seed(&["abc", "def"]);
    app.caret_column = 3;
    app.delete_forward();
    check("join lines", &app.lines, &["abcdef"], &mut failures);
    app.caret_column = 0;
    app.delete_forward();
    check("delete first character", &app.lines, &["bcdef"], &mut failures);
    app.undo();
    check("delete undo", &app.lines, &["abcdef"], &mut failures);

    let mut app = App::new();
    app.seed(&["fn main() {{", "    cons", "}}"]);
    app.caret_line = 1;
    app.caret_column = 8;
    app.accept_completion();
    if !app.lines[1].starts_with("    const") {{
        failures.push(format!("completion: got {{:?}}", app.lines[1]));
    }}
    app.undo();
    check("completion undo", &app.lines, &["fn main() {{", "    cons", "}}"], &mut failures);

    let mut app = App::new();
    app.project_entries = ["src", "src/a.rs", "src/b.rs", "README.md"]
        .iter()
        .map(|value| (*value).to_string())
        .collect();
    app.project_entry_paths = app
        .project_entries
        .iter()
        .map(std::path::PathBuf::from)
        .collect();
    app.update_tree_rows(1.0 / 60.0);
    if app.tree_row_positions != [0.0, 1.0, 2.0, 3.0] {{
        failures.push(format!("tree rows start on their place: {{:?}}", app.tree_row_positions));
    }}
    // `src` folds, so its two children leave the list and the last row slides up.
    app.project_entries = ["src", "README.md"].iter().map(|value| (*value).to_string()).collect();
    app.project_entry_paths = app.project_entries.iter().map(std::path::PathBuf::from).collect();
    app.update_tree_rows(1.0 / 60.0);
    let sliding = app.tree_row_position(1);
    if !(sliding > 1.0 && sliding < 3.0) {{
        failures.push(format!("the row below the fold should slide, got {{sliding}}"));
    }}
    for _ in 0..240 {{
        app.update_tree_rows(1.0 / 60.0);
    }}
    if app.tree_row_positions != [0.0, 1.0] {{
        failures.push(format!("tree rows settle after the fold: {{:?}}", app.tree_row_positions));
    }}
    // Unfolding pushes the same row back down.
    app.project_entries = ["src", "src/a.rs", "src/b.rs", "README.md"]
        .iter()
        .map(|value| (*value).to_string())
        .collect();
    app.project_entry_paths = app.project_entries.iter().map(std::path::PathBuf::from).collect();
    app.update_tree_rows(1.0 / 60.0);
    if app.tree_row_position(1) >= 3.0 {{
        failures.push("an unfolded row should slide down, not jump".to_string());
    }}
    for _ in 0..240 {{
        app.update_tree_rows(1.0 / 60.0);
    }}
    if app.tree_row_positions != [0.0, 1.0, 2.0, 3.0] {{
        failures.push(format!("tree rows settle after unfolding: {{:?}}", app.tree_row_positions));
    }}

    if failures.is_empty() {{
        println!("all editor sequences behave as expected");
    }} else {{
        for failure in &failures {{
            println!("FAILED {{failure}}");
        }}
        std::process::exit(1);
    }}
}}
'''


def source_files(repo: str) -> list:
    """Every file the Windows app is made of: the module root plus its children."""
    root = os.path.join(repo, "src", "windows_app.rs")
    paths = [root]
    directory = os.path.join(repo, "src", "windows_app")
    if os.path.isdir(directory):
        paths += [os.path.join(directory, name) for name in sorted(os.listdir(directory))]
    return [path for path in paths if path.endswith(".rs")]


def extract_methods(source: str) -> str:
    def extract(name: str) -> str:
        match = re.search(rf"^    (?:pub\(super\) )?fn {re.escape(name)}\(", source, re.M)
        if match is None:
            raise SystemExit(f"editor sim: {name} is not defined as an inherent method")
        start = match.start()
        depth = 0
        index = source.index("{", start)
        while True:
            if source[index] == "{":
                depth += 1
            elif source[index] == "}":
                depth -= 1
                if depth == 0:
                    break
            index += 1
        found = source[start:index + 1]
        # The methods are `pub(super)` inside the app module; the scratch crate
        # has no parent module, so the visibility prefix has to go.
        return found.replace("    pub(super) fn ", "    fn ")

    return "\n\n".join(extract(name) for name in METHODS)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--work", default="/tmp/tc-ide-editor-sim")
    parser.add_argument("--repo", default=os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    arguments = parser.parse_args()

    source = "\n".join(open(path).read() for path in source_files(arguments.repo))
    methods = extract_methods(source)

    shutil.rmtree(arguments.work, ignore_errors=True)
    os.makedirs(os.path.join(arguments.work, "src"), exist_ok=True)
    with open(os.path.join(arguments.work, "Cargo.toml"), "w") as handle:
        handle.write('[package]\nname = "editor-sim"\nversion = "0.1.0"\nedition = "2021"\n')
    with open(os.path.join(arguments.work, "src", "main.rs"), "w") as handle:
        handle.write(
            HARNESS.format(
                buffer=os.path.join(arguments.repo, "src", "text_buffer.rs"),
                editor_core=os.path.join(arguments.repo, "src", "editor_core.rs"),
                motion_core=os.path.join(arguments.repo, "src", "motion_core.rs"),
                methods=methods,
            )
        )
    result = subprocess.run(
        ["cargo", "run", "--quiet"], cwd=arguments.work, capture_output=True, text=True
    )
    for line in result.stdout.splitlines():
        if line.startswith(("FAILED", "all editor")):
            print(line)
    if result.returncode != 0:
        if not result.stdout.strip():
            print(result.stderr[-2000:])
        return result.returncode
    return 0


if __name__ == "__main__":
    sys.exit(main())
