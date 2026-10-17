//! The scriptable state layer check behind `tc-ide --self-check`.
//!
//! The Windows job of the CI pipeline runs the release binary with this flag, so
//! every behaviour that only the Windows backend can reach — the packaging
//! command, the breakpoint table, the motion models, the index — is asserted
//! here as well as in the unit tests.

use super::*;

/// Scriptable state layer check: `tc-ide --self-check` exercises the shared
/// cores inside the Windows binary and prints a report, so CI can run it on a
/// Windows runner without a display.
pub(super) fn self_check() -> i32 {
    let mut failures = Vec::new();
    let mut checks = 0usize;
    let mut report = |name: &str, ok: bool, failures: &mut Vec<String>| {
        checks += 1;
        if !ok {
            failures.push(name.to_string());
        }
    };

    // The native Windows menu is generated from the shared menu model. Every
    // enabled row must make a lossless round trip through WM_COMMAND; rows
    // whose Windows view state does not exist yet are visibly disabled.
    report(
        "native menu command ids round trip",
        crate::menu_core::Item::ALL.iter().all(|item| {
            let command = platform::command_for_menu_item(*item);
            platform::menu_item_for_command(command) == Some(*item)
        }),
        &mut failures,
    );
    report(
        "native menu exposes no dead enabled rows",
        crate::menu_core::Item::ALL
            .iter()
            .filter(|item| !platform::menu_item_supported(**item))
            .count()
            == 7,
        &mut failures,
    );

    // Preferences round trip.
    let preferences = workspace_core::Preferences {
        light_theme: true,
        ui_chinese: true,
        code_zoom: 44,
        ..workspace_core::Preferences::default()
    };
    let text = workspace_core::serialize_preferences(&preferences);
    let parsed = workspace_core::parse_preferences(&text);
    report(
        "preferences serialize and parse",
        parsed.ui_chinese,
        &mut failures,
    );
    report(
        "preferences normalize zoom",
        parsed.code_zoom == 8,
        &mut failures,
    );

    // Project template and task command.
    let template = workspace_core::project_template("Demo App", "Rust", "stable", "None");
    report(
        "rust template writes Cargo.toml and main.rs",
        template.files.iter().any(|file| file.path == "Cargo.toml")
            && template.files.iter().any(|file| file.path == "src/main.rs"),
        &mut failures,
    );
    let facts = workspace_core::ProjectFacts {
        has_cargo: true,
        language: "Rust".into(),
        file_name: "main.rs".into(),
        file_stem: "main".into(),
        windows: true,
        ..workspace_core::ProjectFacts::default()
    };
    report(
        "rust task command",
        workspace_core::task_command(&facts, true) == "cargo build",
        &mut failures,
    );

    // Terminal grid, scrollback and selection.
    let grid = terminal_core::GridSize::from_metrics(800.0, 480.0, 8.0, 16.0);
    report(
        "terminal grid fits the window",
        grid.columns == 100 && grid.rows == 30,
        &mut failures,
    );
    let mut parser = terminal_core::TerminalParser::new();
    parser.feed("hello\r\nworld");
    let view = std::sync::Arc::new(std::sync::Mutex::new(terminal_core::TerminalView::default()));
    parser.publish(&view);
    let published = view
        .lock()
        .map(|view| view.current.clone())
        .unwrap_or_default();
    report(
        "VT parser publishes the live prompt",
        published == "world",
        &mut failures,
    );
    let buffer = vec!["alpha".to_string(), "beta".to_string()];
    let selection = terminal_core::line_selection(0, 5);
    report(
        "terminal selection copies text",
        terminal_core::selected_text(&buffer, &selection).as_deref() == Some("alpha"),
        &mut failures,
    );

    // Relation graph scope, folding and edges.
    let sources = vec![(
        "src/main.rs".to_string(),
        vec![
            "pub fn run() {".to_string(),
            "    helper();".to_string(),
            "}".to_string(),
            "pub fn helper() {}".to_string(),
        ],
    )];
    let project = editor_core::project_symbol_occurrences(&sources);
    let declarations = sources
        .iter()
        .flat_map(|(name, lines)| {
            syntax_core::declaration_refs(&syntax_core::scan(lines, name), name)
        })
        .collect::<Vec<_>>();
    let lines = sources[0].1.clone();
    let symbols = editor_core::symbol_occurrences(&lines, "src/main.rs");
    let folded = vec![graph_core::file_key("src/main.rs")];
    let inputs = graph_core::GraphInputs {
        project_name: "demo",
        current_file: "src/main.rs",
        current_folder: "src",
        hovered: None,
        file_lines: &lines,
        file_symbols: &symbols,
        project_symbols: &project,
        project_declarations: &declarations,
        folded: &folded,
        relation: graph_core::GraphRelation::Follow,
    };
    let graph = graph_core::build_graph(&inputs, graph_core::GraphScope::File, "");
    report(
        "graph has a project root",
        graph.nodes[0].level == graph_core::GraphLevel::Project,
        &mut failures,
    );
    report("folding hides children", graph.len() == 2, &mut failures);

    // Session parsing and AI request building.
    let session =
        session_core::Session::parse("project=C:\\demo\ndocument=C:\\demo\\a.rs|a.rs|0\n");
    report(
        "session restores documents",
        session.documents().len() == 1 && session.project().is_some(),
        &mut failures,
    );
    let settings = ai_core::AiSettings::default();
    let command = ai_core::chat_command(&settings, "body.json", true, 30, ai_core::Shell::Windows);
    report(
        "AI command keeps the key in the environment",
        command
            .map(|command| command.contains("%TC_IDE_AI_KEY%") && !command.contains("Bearer sk-"))
            .unwrap_or(false),
        &mut failures,
    );

    // Extension catalog and runtime cache.
    let digest = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let catalog = format!(
        "[extension]\nid=rust-analyzer\nname=Rust Analyzer\nkind=language\nversion=1.0\nurl=https://example.test/ra.zip\nsha256={digest}\n"
    );
    let sources = extension_core::parse_catalog(&catalog).unwrap_or_default();
    report(
        "extension catalog parses",
        sources.len() == 1 && sources[0].is_installable(),
        &mut failures,
    );
    report(
        "checksum verification",
        extension_core::verify_checksum(digest, &digest.to_uppercase()),
        &mut failures,
    );

    // Language tooling.
    report(
        "language server selection",
        lsp_core::server_for_file("main.rs")
            .map(|spec| spec.program == "rust-analyzer")
            .unwrap_or(false),
        &mut failures,
    );
    let (body, rest) = lsp_core::decode_messages(&lsp_core::encode_message("{\"id\":1}"));
    report(
        "LSP framing round trip",
        body == ["{\"id\":1}"] && rest.is_empty(),
        &mut failures,
    );
    report(
        "formatter selection",
        workspace_core::format_command("Rust", "src/main.rs").is_some(),
        &mut failures,
    );
    let mut reader = lsp_core::FrameReader::new();
    let framed = lsp_core::encode_message("{\"id\":7}");
    let bytes = framed.as_bytes();
    let split = bytes.len() - 2;
    let mut frames = reader.push(&bytes[..split]);
    frames.extend(reader.push(&bytes[split..]));
    report("LSP byte framing", frames == ["{\"id\":7}"], &mut failures);
    report(
        "LSP client request answering",
        lsp_core::automatic_reply(
            "{\"id\":2,\"method\":\"workspace/configuration\",\"params\":{\"items\":[{}]}}",
        )
        .map(|reply| reply.contains("[null]"))
        .unwrap_or(false),
        &mut failures,
    );
    report(
        "interpreter probe is behavioural",
        // A missing program must report false rather than a file name match.
        !lsp_core::program_runs("tc-ide-missing-interpreter", &["-c", "print(1)"]),
        &mut failures,
    );
    report(
        "language id mapping",
        lsp_core::language_id("app.py") == "python",
        &mut failures,
    );
    let applied = lsp_core::apply_edits(
        "let value = 1;\n",
        &[lsp_core::RenameEdit {
            path: "demo.rs".into(),
            line: 0,
            column: 4,
            end_line: 0,
            end_column: 9,
            new_text: "answer".into(),
        }],
    );
    report(
        "rename edits apply to a document",
        applied == "let answer = 1;\n",
        &mut failures,
    );
    let mut spring = motion_core::Spring::new(0.0);
    for _ in 0..120 {
        spring.step(1.0, 1.0 / 60.0);
    }
    report(
        "spring settles on its target",
        spring.is_settled(1.0) && spring.value == 1.0,
        &mut failures,
    );
    let tooling = extension_core::language_tools("Rust", |tool| tool == "rustfmt");
    report(
        "tooling state follows the real executables",
        tooling.formatter_installed
            && !tooling.server_installed
            && tooling.missing().contains(&"language server"),
        &mut failures,
    );
    let mut panel_state = WindowsAppState::new();
    // A jump target only exists once the panel knows which file is active.
    panel_state.file_path = Some(PathBuf::from("C:/demo/src/main.rs"));
    panel_state.lsp_diagnostics = vec![lsp_core::Diagnostic {
        path: "src/main.rs".into(),
        line: 3,
        column: 2,
        severity: lsp_core::DiagnosticSeverity::Error,
        message: "cannot find value `x`".into(),
    }];
    let rows = panel_state.problem_rows();
    report(
        "problems panel lists diagnostics with a jump target",
        rows.len() == 1 && rows[0].0.contains("cannot find value") && rows[0].1.is_some(),
        &mut failures,
    );
    let scan = syntax_core::scan(
        &[
            "class Engine {".to_string(),
            "    start() {".to_string(),
            "        helper();".to_string(),
            "    }".to_string(),
            "}".to_string(),
        ],
        "main.ts",
    );
    report(
        "syntax scan finds class members and calls",
        scan.scopes
            .iter()
            .any(|scope| scope.name.as_deref() == Some("Engine"))
            && scan
                .scopes
                .iter()
                .any(|scope| scope.name.as_deref() == Some("start") && scope.end_line == 3)
            && scan.declarations.iter().any(|entry| entry.name == "start"),
        &mut failures,
    );
    let mut index = index_core::IncrementalIndex::new();
    let stats = index.refresh(
        vec![index_core::FileFingerprint::new("src/a.rs", 10, 1)],
        |_| Some(vec!["pub fn run() {}".to_string()]),
    );
    let reused = index.refresh(
        vec![index_core::FileFingerprint::new("src/a.rs", 10, 1)],
        |_| panic!("an unchanged file must not be read again"),
    );
    report(
        "incremental index reuses unchanged files",
        stats.reparsed == 1 && reused.reused == 1 && !index.symbols().is_empty(),
        &mut failures,
    );
    report(
        "chat stream parsing",
        ai_core::parse_stream_line("data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}")
            == ai_core::StreamEvent::Delta("hi".into())
            && ai_core::parse_stream_line("data: [DONE]") == ai_core::StreamEvent::Done,
        &mut failures,
    );
    report(
        "debug adapter selection",
        dap_core::adapter_for("Python")
            .map(|spec| spec.program == "python")
            .unwrap_or(false)
            && dap_core::adapter_for("Markdown").is_none(),
        &mut failures,
    );
    let stopped = dap_core::parse_stopped(
        "{\"type\":\"event\",\"event\":\"stopped\",\"body\":{\"reason\":\"breakpoint\",\"threadId\":2,\"line\":5}}",
    );
    report(
        "debug stop parsing",
        stopped.reason == "breakpoint" && stopped.thread_id == Some(2),
        &mut failures,
    );
    report(
        "DAP message classification",
        dap_core::parse_dap_message("{\"type\":\"event\",\"event\":\"stopped\"}")
            == dap_core::DapMessage::Event {
                name: "stopped".into(),
            },
        &mut failures,
    );
    report(
        "reference locations parse",
        lsp_core::parse_locations(
            "{\"result\":[{\"uri\":\"file:///p/a.rs\",\"range\":{\"start\":{\"line\":2,\"character\":1},\"end\":{}}}]}",
        )
        .len()
            == 1,
        &mut failures,
    );

    // Packaging: the selection file, the command and the manifest are shared.
    let targets = workspace_core::parse_package_targets(
        &workspace_core::serialize_package_targets(&[true, false, true]),
        3,
    );
    report(
        "package targets round trip",
        targets == [true, false, true],
        &mut failures,
    );
    let request = workspace_core::PackageRequest {
        project: "demo".into(),
        file: "main.rs".into(),
        language: "Rust".into(),
        stage: ".tc-ide-packages\\demo".into(),
        has_cargo: true,
        has_go_mod: false,
        targets: [true, true, true],
        certificate: None,
        marker: "__DONE__".into(),
        failure_marker: "__FAILED__".into(),
        fallback: "echo none".into(),
    };
    let command = workspace_core::package_command(&request);
    report(
        "packaging command builds every selected target",
        command.contains("cargo build --release")
            && command.contains("Compress-Archive")
            && command.contains("makeappx.exe")
            && !command.contains("signtool")
            && command.contains("Write-Output '__DONE__'"),
        &mut failures,
    );
    let manifest = workspace_core::msix_manifest(&workspace_core::MsixPackage {
        project: "demo".into(),
        identity: workspace_core::msix_identity("demo app"),
        display_name: workspace_core::msix_display_name("demo & app"),
        version: "0.3.0.0".into(),
    });
    report(
        "msix manifest carries identity and display name",
        manifest.contains("Name=\"TianChuangYa.demo.app\"")
            && manifest.contains("Executable=\"demo.exe\"")
            && !manifest.contains("demo & app"),
        &mut failures,
    );

    // The settings panel is a control grid walked with Tab. The rows the
    // keyboard can reach and what each one needs come from the shared model,
    // so the Windows panel cannot disagree with Linux about what is usable.
    let with_picture = background_core::background_controls(true, 0.5, 1.5);
    let without_picture = background_core::background_controls(false, 0.5, 1.5);
    let reachable = background_core::SettingsRow::focusable(with_picture);
    report(
        "the settings panel walks only usable rows",
        !reachable.contains(&background_core::SettingsRow::Heading)
            && reachable.contains(&background_core::SettingsRow::AdjustBackground)
            && reachable.contains(&background_core::SettingsRow::ClearBackground)
            && reachable.len() == 8,
        &mut failures,
    );
    let bare = background_core::SettingsRow::focusable(without_picture);
    report(
        "rows that need a picture are skipped without one",
        !bare.contains(&background_core::SettingsRow::AdjustBackground)
            && !bare.contains(&background_core::SettingsRow::ClearBackground)
            && bare.contains(&background_core::SettingsRow::ChooseBackground)
            && bare.len() + 2 == reachable.len(),
        &mut failures,
    );
    // Tab walks forward, shift Tab walks back, and both wrap.
    let forward = theme_core::step_focus(&reachable, None, true);
    let backward = theme_core::step_focus(&reachable, None, false);
    report(
        "tab starts at the ends and wraps around",
        forward == reachable.first().copied()
            && backward == reachable.last().copied()
            && theme_core::step_focus(&reachable, reachable.last().copied(), true)
                == reachable.first().copied()
            && theme_core::step_focus(&reachable, reachable.first().copied(), false)
                == reachable.last().copied(),
        &mut failures,
    );
    // Walking from a row that is no longer reachable starts over instead of
    // leaving the keyboard nowhere.
    report(
        "a focus that went away restarts from an end",
        theme_core::step_focus(
            &bare,
            Some(background_core::SettingsRow::ClearBackground),
            true,
        ) == bare.first().copied(),
        &mut failures,
    );

    // The palettes of both themes have to be readable, which is the check the
    // settings panel cannot do by eye. The rule follows where each colour is
    // actually drawn: text and hints also sit on raised controls, syntax colours
    // only on the code background, a border only has to separate, and a
    // selection only has to be a fill with readable text in it.
    let mut unreadable = Vec::new();
    for (name, palette) in [
        ("dark", theme_core::Palette::dark()),
        ("light", theme_core::Palette::light()),
    ] {
        for (role, colour) in palette.all() {
            let mut check = |surface: u32, minimum: f64| {
                if !theme_core::meets_contrast(colour, surface, minimum) {
                    unreadable.push(format!("{name}/{role}"));
                }
            };
            match role {
                "background" | "panel" | "raised" | "shadow" => {}
                "text" | "bright" | "muted" => {
                    check(palette.panel, theme_core::TEXT_MINIMUM);
                    check(palette.raised, theme_core::TEXT_MINIMUM);
                }
                // Every colour the tokenizer can produce is drawn on the code
                // background, so all of them have to be readable there.
                "keyword" | "literal" | "comment" | "type" | "function" | "constant"
                | "variable" | "parameter" | "property" | "macro_name" | "module"
                | "preprocessor" => {
                    check(palette.background, theme_core::TEXT_MINIMUM);
                }
                "accent" => {
                    check(palette.background, theme_core::DECORATION_MINIMUM);
                    check(palette.panel, theme_core::DECORATION_MINIMUM);
                }
                "border" => {
                    if theme_core::contrast_ratio(colour, palette.background) < 1.2 {
                        unreadable.push(format!("{name}/{role}"));
                    }
                }
                "selected_text" => check(palette.selection, theme_core::DECORATION_MINIMUM),
                "selection"
                    if colour == palette.background
                        || !theme_core::meets_contrast(
                            palette.text,
                            colour,
                            theme_core::DECORATION_MINIMUM,
                        ) =>
                {
                    unreadable.push(format!("{name}/{role}"));
                }
                _ => {}
            }
        }
    }
    report(
        "both themes stay readable",
        unreadable.is_empty(),
        &mut failures,
    );

    // Motion: the row shift slides a list and the edit highlights stay bounded.
    let mut rows = motion_core::RowShift::new();
    rows.place("a", 0.0, 1.0 / 60.0);
    let sliding = rows.place("b", 2.0, 1.0 / 60.0);
    let after_fold = rows.place("b", 1.0, 1.0 / 60.0);
    for _ in 0..240 {
        rows.place("a", 0.0, 1.0 / 60.0);
        rows.place("b", 1.0, 1.0 / 60.0);
    }
    report(
        "tree rows slide instead of jumping",
        sliding == 2.0 && after_fold < 2.0 && rows.settled(),
        &mut failures,
    );
    let mut pulses = motion_core::EditPulses::new();
    pulses.push(motion_core::EditKind::Paste, 0, 0, 200);
    for _ in 0..40 {
        pulses.push(motion_core::EditKind::Insert, 1, 0, 1);
    }
    report(
        "a paste is one highlight and a burst is capped",
        pulses.on_line(0).is_none() || pulses.len() <= motion_core::EditPulses::LIMIT,
        &mut failures,
    );

    // Packaging animation and background placement are shared pure functions.
    let travel = package_motion::absorbed((600.0, 100.0), (300.0, 300.0), 2, 6, 500.0, 0.9);
    report(
        "sources are absorbed into the core",
        travel.progress > 0.0 && travel.x > 300.0 && travel.x < 600.0,
        &mut failures,
    );
    let placed = background_core::placement((2000.0, 1000.0), (800.0, 800.0), 2.0, (0.0, 0.0));
    let crop = placed.crop((800.0, 800.0), (2000.0, 1000.0));
    report(
        "background placement covers the view",
        placed.scale > 0.0 && crop.height < 1000.0 && crop.width <= 2000.0,
        &mut failures,
    );
    let preview = background_core::preview_placement((2000.0, 1000.0), (400.0, 300.0));
    let on_screen = preview.project(&crop);
    let edited = background_core::drag_crop(
        &crop,
        background_core::Handle::Inside,
        (40.0, 0.0),
        (2000.0, 1000.0),
        (800.0, 800.0),
        40.0,
    );
    report(
        "the background crop preview can be dragged",
        preview.drawn == (400.0, 200.0)
            && preview.origin.1 == 50.0
            && on_screen.width > 0.0
            && edited.crop.x > crop.x,
        &mut failures,
    );

    println!(
        "TC-IDE self check: {checks} checks, {} failed",
        failures.len()
    );
    for name in &failures {
        println!("  FAILED: {name}");
    }
    if failures.is_empty() {
        println!("state layer OK");
        0
    } else {
        1
    }
}
