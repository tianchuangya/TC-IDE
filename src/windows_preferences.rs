use std::fs;
use std::path::PathBuf;

use crate::workspace_core::{self, Preferences};

/// Preferences are the shared workspace model; only the file path is Windows
/// specific.
pub type WindowsPreferences = Preferences;

fn path() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("TC-IDE")
        .join("settings.conf")
}

pub fn load() -> WindowsPreferences {
    match fs::read_to_string(path()) {
        Ok(content) => workspace_core::parse_preferences(&content),
        Err(_) => Preferences::default(),
    }
}

pub fn save(preferences: &WindowsPreferences) -> std::io::Result<()> {
    let path = path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, workspace_core::serialize_preferences(preferences))
}
