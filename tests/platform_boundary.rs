//! Guard the shared cores against platform dependencies.
//!
//! The handoff requires that the shared modules stay platform independent: they
//! are compiled for Linux, Windows and macOS and unit tested on Linux, so a
//! single Win32, X11 or Cocoa reference would either break another platform or
//! hide behind a `cfg` that never runs locally. This test reads the shared
//! sources and refuses the imports, handles and unsafe platform calls that do
//! not belong there.

use std::fs;
use std::path::PathBuf;

/// Modules that must stay platform independent.
const SHARED_CORES: [&str; 16] = [
    "ai_core.rs",
    "background_core.rs",
    "dap_core.rs",
    "editor_core.rs",
    "extension_core.rs",
    "graph_core.rs",
    "index_core.rs",
    "lsp_core.rs",
    "motion_core.rs",
    "package_motion.rs",
    "session_core.rs",
    "syntax_core.rs",
    "terminal_core.rs",
    "theme_core.rs",
    "text_buffer.rs",
    "workspace_core.rs",
];

/// `(needle, why it does not belong in a shared core)`.
const FORBIDDEN: [(&str, &str); 12] = [
    ("windows::", "the Win32 bindings"),
    ("Hwnd", "a Win32 window handle"),
    ("HINSTANCE", "a Win32 module handle"),
    ("XOpenDisplay", "X11"),
    ("cairo_", "Cairo"),
    ("pango_", "Pango"),
    ("objc", "Objective-C"),
    ("NSPasteboard", "AppKit"),
    ("CoreText", "CoreText"),
    ("extern \"system\"", "a raw platform call"),
    ("std::os::raw", "raw platform types"),
    ("libc::", "the C library"),
];

fn source(name: &str) -> (PathBuf, String) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join(name);
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} is not readable: {error}", path.display()));
    (path, content)
}

#[test]
fn shared_cores_do_not_reach_for_a_platform_api() {
    for name in SHARED_CORES {
        let (path, content) = source(name);
        for (needle, why) in FORBIDDEN {
            let offending = content
                .lines()
                .enumerate()
                .filter(|(_, line)| line.contains(needle))
                .map(|(index, line)| format!("{}:{}: {}", path.display(), index + 1, line.trim()))
                .collect::<Vec<_>>();
            // `unsafe` also appears as a Rust keyword in the language tables, so
            // only real keywords are reported.
            assert!(
                offending.is_empty(),
                "{name} must not use {why}:\n{}",
                offending.join("\n")
            );
        }
    }
}

#[test]
fn shared_cores_have_no_unsafe_blocks() {
    for name in SHARED_CORES {
        let (path, content) = source(name);
        let offenders = content
            .lines()
            .enumerate()
            .filter(|(_, line)| {
                let trimmed = line.trim();
                trimmed.starts_with("unsafe ")
                    || trimmed.contains("unsafe fn ")
                    || trimmed == "unsafe"
            })
            .map(|(index, line)| format!("{}:{}: {}", path.display(), index + 1, line.trim()))
            .collect::<Vec<_>>();
        assert!(
            offenders.is_empty(),
            "{name} must stay safe Rust:\n{}",
            offenders.join("\n")
        );
    }
}

#[test]
fn shared_cores_are_declared_in_main() {
    let (_, main) = source("main.rs");
    for name in SHARED_CORES {
        let module = name.trim_end_matches(".rs");
        assert!(
            main.contains(&format!("mod {module};")),
            "main.rs does not declare {module}, so this guard would not notice it"
        );
    }
}

#[test]
fn platform_modules_are_kept_behind_a_target() {
    let (_, main) = source("main.rs");
    for module in ["windows_app", "linux_app", "macos_app"] {
        let declaration = format!("mod {module};");
        let position = main
            .find(&declaration)
            .unwrap_or_else(|| panic!("main.rs does not declare {module}"));
        let before = &main[..position];
        let attribute = before
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .unwrap_or_default();
        assert!(
            attribute.contains("cfg(target_os"),
            "{module} must stay behind a cfg(target_os) attribute, found {attribute:?}"
        );
    }
}
