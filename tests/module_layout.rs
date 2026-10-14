//! Guard the layout of the Windows app and of its renderer.
//!
//! `windows_app.rs` used to carry every message, editor command, panel and
//! painter in one file of more than seven thousand lines, and the renderer kept
//! a single function of more than two thousand lines. Both were split into
//! regions: window events, keys, pointer, editor, workspace, language,
//! extensions, packaging, terminal panel, detached terminal, scene, motion and
//! AI modules, plus one painter per scene region. This test keeps that split
//! honest, because it is exactly the kind of structure that quietly decays.

use std::fs;
use std::path::PathBuf;

/// `(module, the region it owns)` of the Windows app.
const WINDOWS_MODULES: [(&str, &str); 14] = [
    ("ai", "the AI chat panel and its transcript"),
    ("debug", "DAP sessions, breakpoints and stepping"),
    ("detached", "the standalone ConPTY window"),
    ("editor", "buffer edits, caret, selection and undo"),
    ("events", "the window procedure and command dispatch"),
    ("extensions", "extension catalog, installation and probes"),
    ("keys", "keyboard handling"),
    ("language", "LSP session, diagnostics and the problems panel"),
    ("motion", "spring stepping shared by both backends"),
    ("packaging", "package targets, commands, progress and reports"),
    ("panel", "the bottom terminal panel"),
    ("pointer", "mouse handling per region"),
    ("scene", "scene assembly and graph geometry"),
    ("workspace", "project scanning, templates and sessions"),
];

/// Scene regions the renderer paints separately.
const RENDER_REGIONS: [&str; 8] = [
    "terminal_panel",
    "package_progress",
    "package_overlay",
    "settings_overlay",
    "extensions_overlay",
    "problems_panel",
    "chat_panel",
    "create_dialog",
];

/// A file may grow this large before it should be split again. The app root
/// carries the Win32 declarations and the application state, so it is allowed
/// more room than a region module.
const ROOT_LIMIT: usize = 1500;
const MODULE_LIMIT: usize = 1000;

fn source(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join(relative);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} is not readable: {error}", path.display()))
}

fn line_count(text: &str) -> usize {
    text.lines().count()
}

#[test]
fn the_windows_app_is_split_into_region_modules() {
    let root = source("windows_app.rs");
    for (module, region) in WINDOWS_MODULES {
        assert!(
            root.contains(&format!("mod {module};")),
            "windows_app.rs does not declare {module}, which owns {region}"
        );
        let child = source(&format!("windows_app/{module}.rs"));
        let first = child
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or_default();
        assert!(
            first.starts_with("//! "),
            "windows_app/{module}.rs must open with a module comment, found {first:?}"
        );
        assert!(
            child.contains("use super::*;"),
            "windows_app/{module}.rs must take the app items from its parent module"
        );
    }
}

#[test]
fn no_windows_source_file_grows_back_into_a_monolith() {
    let root = source("windows_app.rs");
    assert!(
        line_count(&root) <= ROOT_LIMIT,
        "windows_app.rs is {} lines; move the next region into a module below src/windows_app/",
        line_count(&root)
    );
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/windows_app");
    let mut checked = 0;
    for entry in fs::read_dir(&directory).expect("src/windows_app exists") {
        let path = entry.expect("readable entry").path();
        if path.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }
        let text = fs::read_to_string(&path).expect("readable module");
        checked += 1;
        assert!(
            line_count(&text) <= MODULE_LIMIT,
            "{} is {} lines; split it by scene region",
            path.display(),
            line_count(&text)
        );
    }
    assert_eq!(
        checked,
        WINDOWS_MODULES.len(),
        "every declared region module should be a file in src/windows_app/"
    );
}

#[test]
fn the_renderer_keeps_one_painter_per_scene_region() {
    let renderer = source("windows_renderer.rs");
    assert!(
        renderer.contains("struct ScenePaint<"),
        "the renderer should hand the frame state to the region painters"
    );
    for region in RENDER_REGIONS {
        assert!(
            renderer.contains(&format!("fn draw_{region}(&self)")),
            "windows_renderer.rs has no painter for the {region} region"
        );
        assert!(
            renderer.contains(&format!("paint.draw_{region}();")),
            "render does not call the {region} painter"
        );
    }
}
