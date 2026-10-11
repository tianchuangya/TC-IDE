use std::fs;
use std::path::PathBuf;

#[derive(Clone, Default)]
pub struct WindowsPreferences {
    pub light_theme: bool,
    pub ui_chinese: bool,
    pub code_zoom: i32,
    pub project_language: usize,
    pub project_environment: usize,
}

fn path() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("TC-IDE")
        .join("settings.conf")
}

pub fn load() -> WindowsPreferences {
    let mut preferences = WindowsPreferences::default();
    let Ok(content) = fs::read_to_string(path()) else {
        return preferences;
    };
    for line in content.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key {
            "theme" => preferences.light_theme = value == "light",
            "language" => preferences.ui_chinese = value == "zh-CN",
            "code_zoom" => preferences.code_zoom = value.parse().unwrap_or(0).clamp(-2, 8),
            "project_language" => preferences.project_language = value.parse().unwrap_or(0),
            "project_environment" => preferences.project_environment = value.parse().unwrap_or(0),
            _ => {}
        }
    }
    preferences
}

pub fn save(preferences: &WindowsPreferences) -> std::io::Result<()> {
    let path = path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        path,
        format!(
            "theme={}\nlanguage={}\ncode_zoom={}\nproject_language={}\nproject_environment={}\n",
            if preferences.light_theme {
                "light"
            } else {
                "dark"
            },
            if preferences.ui_chinese {
                "zh-CN"
            } else {
                "en"
            },
            preferences.code_zoom,
            preferences.project_language,
            preferences.project_environment,
        ),
    )
}
