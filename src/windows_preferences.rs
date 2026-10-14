use std::fs;
use std::path::PathBuf;

#[derive(Clone, Default)]
pub struct WindowsPreferences {
    pub light_theme: bool,
    pub ui_chinese: bool,
    pub code_zoom: i32,
    pub project_language: usize,
    pub project_environment: usize,
    pub language_installed: Vec<bool>,
    pub language_versions: Vec<usize>,
    pub background_path: Option<PathBuf>,
    pub background_opacity: f32,
    pub background_offset_x: f32,
    pub background_offset_y: f32,
    pub background_zoom: f32,
    pub ai_endpoint: String,
    pub ai_model: String,
}

impl WindowsPreferences {
    fn normalize_background(&mut self) {
        self.background_opacity = if self.background_opacity == 0.0 {
            0.24
        } else {
            self.background_opacity.clamp(0.05, 1.0)
        };
        self.background_offset_x = self.background_offset_x.clamp(-1.0, 1.0);
        self.background_offset_y = self.background_offset_y.clamp(-1.0, 1.0);
        self.background_zoom = if self.background_zoom == 0.0 {
            1.0
        } else {
            self.background_zoom.clamp(1.0, 3.0)
        };
    }
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
    preferences.ai_endpoint = "https://api.openai.com/v1".into();
    preferences.ai_model = "gpt-5.1".into();
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
            "language_installed" => {
                preferences.language_installed = value.split(',').map(|item| item == "1").collect()
            }
            "language_versions" => {
                preferences.language_versions = value
                    .split(',')
                    .map(|item| item.parse().unwrap_or(0))
                    .collect()
            }
            "background_path" if !value.is_empty() => {
                preferences.background_path = Some(PathBuf::from(value))
            }
            "background_opacity" => preferences.background_opacity = value.parse().unwrap_or(0.24),
            "background_offset_x" => preferences.background_offset_x = value.parse().unwrap_or(0.0),
            "background_offset_y" => preferences.background_offset_y = value.parse().unwrap_or(0.0),
            "background_zoom" => preferences.background_zoom = value.parse().unwrap_or(1.0),
            "ai_endpoint" => preferences.ai_endpoint = value.to_string(),
            "ai_model" => preferences.ai_model = value.to_string(),
            _ => {}
        }
    }
    preferences.normalize_background();
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
            "theme={}\nlanguage={}\ncode_zoom={}\nproject_language={}\nproject_environment={}\nlanguage_installed={}\nlanguage_versions={}\nbackground_path={}\nbackground_opacity={}\nbackground_offset_x={}\nbackground_offset_y={}\nbackground_zoom={}\nai_endpoint={}\nai_model={}\n",
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
            preferences
                .language_installed
                .iter()
                .map(|value| if *value { "1" } else { "0" })
                .collect::<Vec<_>>()
                .join(","),
            preferences
                .language_versions
                .iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(","),
            preferences
                .background_path
                .as_ref()
                .map(|path| path.to_string_lossy())
                .unwrap_or_default(),
            preferences.background_opacity,
            preferences.background_offset_x,
            preferences.background_offset_y,
            preferences.background_zoom,
            preferences.ai_endpoint.replace(['\n', '\r'], ""),
            preferences.ai_model.replace(['\n', '\r'], ""),
        ),
    )
}
