//! Shared workspace model: preferences, project templates and task commands.
//!
//! Both native backends keep their own settings file path and their own way of
//! writing files, but the file format, the starter templates and the command
//! lines a project needs are plain data. Keeping them here means Linux
//! `cargo test` covers parsing, templates and commands instead of leaving them
//! to a Windows CI job or to real machines.
#![allow(dead_code)] // The backends adopt the pieces they need as they migrate.

/// Endpoint offered before the user configures one.
pub const DEFAULT_AI_ENDPOINT: &str = "https://api.openai.com/v1";
/// Model offered before the user configures one.
pub const DEFAULT_AI_MODEL: &str = "gpt-5.1";
/// Background opacity used when the file does not carry one.
pub const DEFAULT_BACKGROUND_OPACITY: f32 = 0.24;

/// Editor, project and AI preferences shared by the backends.
#[derive(Clone, Debug, PartialEq)]
pub struct Preferences {
    pub light_theme: bool,
    pub ui_chinese: bool,
    pub code_zoom: i32,
    pub code_font_family: usize,
    pub project_language: usize,
    pub project_environment: usize,
    pub language_installed: Vec<bool>,
    pub language_versions: Vec<usize>,
    pub background_path: Option<String>,
    pub background_opacity: f32,
    pub background_offset_x: f32,
    pub background_offset_y: f32,
    pub background_zoom: f32,
    pub ai_endpoint: String,
    pub ai_model: String,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            light_theme: false,
            ui_chinese: false,
            code_zoom: 0,
            code_font_family: 0,
            project_language: 0,
            project_environment: 0,
            language_installed: Vec::new(),
            language_versions: Vec::new(),
            background_path: None,
            background_opacity: DEFAULT_BACKGROUND_OPACITY,
            background_offset_x: 0.0,
            background_offset_y: 0.0,
            background_zoom: 1.0,
            ai_endpoint: DEFAULT_AI_ENDPOINT.to_string(),
            ai_model: DEFAULT_AI_MODEL.to_string(),
        }
    }
}

impl Preferences {
    /// Clamp values that the UI and the renderer rely on.
    pub fn normalize(&mut self) {
        self.code_zoom = self.code_zoom.clamp(-2, 8);
        self.background_opacity = if self.background_opacity == 0.0 {
            DEFAULT_BACKGROUND_OPACITY
        } else {
            self.background_opacity.clamp(0.05, 1.0)
        };
        self.background_offset_x = clamp_unit(self.background_offset_x);
        self.background_offset_y = clamp_unit(self.background_offset_y);
        self.background_zoom = if self.background_zoom == 0.0 {
            1.0
        } else {
            self.background_zoom.clamp(1.0, 3.0)
        };
        if self.ai_endpoint.trim().is_empty() {
            self.ai_endpoint = DEFAULT_AI_ENDPOINT.to_string();
        }
        if self.ai_model.trim().is_empty() {
            self.ai_model = DEFAULT_AI_MODEL.to_string();
        }
    }
}

fn clamp_unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(-1.0, 1.0)
    } else {
        0.0
    }
}

/// Parse a `key=value` settings file. Unknown keys and broken lines are ignored.
pub fn parse_preferences(content: &str) -> Preferences {
    let mut preferences = Preferences::default();
    for line in content.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        match key {
            "theme" => preferences.light_theme = value.trim() == "light",
            "language" => preferences.ui_chinese = value.trim() == "zh-CN",
            "code_zoom" => preferences.code_zoom = value.trim().parse().unwrap_or(0).clamp(-2, 8),
            // Linux writes `font_family`, Windows writes `code_font_family`.
            "font_family" | "code_font_family" => {
                preferences.code_font_family = value.trim().parse().unwrap_or(0)
            }
            "project_language" => preferences.project_language = value.trim().parse().unwrap_or(0),
            "project_environment" => {
                preferences.project_environment = value.trim().parse().unwrap_or(0)
            }
            "language_installed" => {
                preferences.language_installed =
                    value.split(',').map(|item| item.trim() == "1").collect()
            }
            "language_versions" => {
                preferences.language_versions = value
                    .split(',')
                    .map(|item| item.trim().parse().unwrap_or(0))
                    .collect()
            }
            // Linux writes the background and AI keys without their prefix.
            "background_path" | "path" if !value.trim().is_empty() => {
                preferences.background_path = Some(value.trim().to_string())
            }
            "background_opacity" | "opacity" => {
                preferences.background_opacity =
                    value.trim().parse().unwrap_or(DEFAULT_BACKGROUND_OPACITY)
            }
            "background_offset_x" | "offset_x" => {
                preferences.background_offset_x = value.trim().parse().unwrap_or(0.0)
            }
            "background_offset_y" | "offset_y" => {
                preferences.background_offset_y = value.trim().parse().unwrap_or(0.0)
            }
            "background_zoom" | "zoom" => {
                preferences.background_zoom = value.trim().parse().unwrap_or(1.0)
            }
            "ai_endpoint" | "endpoint" => preferences.ai_endpoint = value.trim().to_string(),
            "ai_model" | "model" => preferences.ai_model = value.trim().to_string(),
            _ => {}
        }
    }
    preferences.normalize();
    preferences
}

/// Render the settings file. Secrets are never written here.
pub fn serialize_preferences(preferences: &Preferences) -> String {
    let mut normalized = preferences.clone();
    normalized.normalize();
    format!(
        "theme={}\nlanguage={}\ncode_zoom={}\ncode_font_family={}\nproject_language={}\nproject_environment={}\nlanguage_installed={}\nlanguage_versions={}\nbackground_path={}\nbackground_opacity={}\nbackground_offset_x={}\nbackground_offset_y={}\nbackground_zoom={}\nai_endpoint={}\nai_model={}\n",
        if normalized.light_theme { "light" } else { "dark" },
        if normalized.ui_chinese { "zh-CN" } else { "en" },
        normalized.code_zoom,
        normalized.code_font_family,
        normalized.project_language,
        normalized.project_environment,
        normalized
            .language_installed
            .iter()
            .map(|value| if *value { "1" } else { "0" })
            .collect::<Vec<_>>()
            .join(","),
        normalized
            .language_versions
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(","),
        normalized.background_path.clone().unwrap_or_default(),
        normalized.background_opacity,
        normalized.background_offset_x,
        normalized.background_offset_y,
        normalized.background_zoom,
        single_line(&normalized.ai_endpoint),
        single_line(&normalized.ai_model),
    )
}

fn single_line(value: &str) -> String {
    value.replace(['\n', '\r'], "")
}

/// One starter file of a new project, with a project relative path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectFile {
    pub path: String,
    pub contents: String,
}

/// Starter files and metadata of a new project.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectTemplate {
    pub files: Vec<ProjectFile>,
    pub metadata: String,
}

/// Name of the file recording language, version and environment of a project.
pub const PROJECT_METADATA_FILE: &str = ".tc-ide-project";

/// Metadata written into every new project.
pub fn project_metadata(language: &str, version: &str, environment: &str) -> String {
    format!("language={language}\nversion={version}\nenvironment={environment}\n")
}

/// Turn a folder name into something usable as a package or module name.
pub fn sanitize_ident(name: &str) -> String {
    let mut result = String::new();
    for value in name.chars() {
        if value.is_ascii_alphanumeric() {
            result.push(value.to_ascii_lowercase());
        } else if !result.ends_with('_') {
            result.push('_');
        }
    }
    let trimmed = result.trim_matches('_').to_string();
    if trimmed.is_empty() {
        "tc_project".to_string()
    } else if trimmed.chars().next().map(|c| c.is_ascii_digit()) == Some(true) {
        format!("tc_{trimmed}")
    } else {
        trimmed
    }
}

/// Starter files for a new project of `language`.
pub fn project_template(
    name: &str,
    language: &str,
    version: &str,
    environment: &str,
) -> ProjectTemplate {
    let ident = sanitize_ident(name);
    let mut files = Vec::new();
    match language {
        "Rust" => {
            files.push(ProjectFile {
                path: "Cargo.toml".into(),
                contents: format!(
                    "[package]\nname = \"{ident}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n"
                ),
            });
            files.push(ProjectFile {
                path: "src/main.rs".into(),
                contents: "fn main() {\n    println!(\"Hello from TC-IDE\");\n}\n".into(),
            });
        }
        "Python" => {
            files.push(ProjectFile {
                path: "main.py".into(),
                contents: "print(\"Hello from TC-IDE\")\n".into(),
            });
            files.push(ProjectFile {
                path: ".gitignore".into(),
                contents: ".venv/\n__pycache__/\n*.pyc\n".into(),
            });
        }
        "C" => files.push(ProjectFile {
            path: "main.c".into(),
            contents: "#include <stdio.h>\n\nint main(void) {\n    puts(\"Hello from TC-IDE\");\n    return 0;\n}\n".into(),
        }),
        "C++" => files.push(ProjectFile {
            path: "main.cpp".into(),
            contents: "#include <iostream>\n\nint main() {\n    std::cout << \"Hello from TC-IDE\\n\";\n    return 0;\n}\n".into(),
        }),
        "Java" => files.push(ProjectFile {
            path: "Main.java".into(),
            contents: "public class Main {\n    public static void main(String[] args) {\n        System.out.println(\"Hello from TC-IDE\");\n    }\n}\n".into(),
        }),
        "Go" => {
            files.push(ProjectFile {
                path: "go.mod".into(),
                contents: format!("module {ident}\n\ngo 1.24\n"),
            });
            files.push(ProjectFile {
                path: "main.go".into(),
                contents:
                    "package main\n\nimport \"fmt\"\n\nfunc main() {\n    fmt.Println(\"Hello from TC-IDE\")\n}\n"
                        .into(),
            });
        }
        _ => files.push(ProjectFile {
            path: "index.html".into(),
            contents: "<!doctype html>\n<title>TC-IDE Project</title>\n<h1>Hello from TC-IDE</h1>\n".into(),
        }),
    }
    ProjectTemplate {
        files,
        metadata: project_metadata(language, version, environment),
    }
}

/// What the workspace knows about a project when it builds a task command.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ProjectFacts {
    pub has_cargo: bool,
    pub has_package_json: bool,
    pub has_go_mod: bool,
    /// Language of the active file, as reported by `editor_core`.
    pub language: String,
    pub file_name: String,
    pub file_stem: String,
    /// Whether the command runs in a Windows shell.
    pub windows: bool,
}

impl ProjectFacts {
    fn quoted_file(&self) -> String {
        let name = if self.file_name.is_empty() {
            "main".to_string()
        } else {
            self.file_name.clone()
        };
        format!("\"{}\"", name.replace('"', "\\\""))
    }

    fn executable(&self) -> String {
        if self.windows {
            ".\\tc-ide-build.exe".to_string()
        } else {
            "./tc-ide-build".to_string()
        }
    }

    fn class_name(&self) -> String {
        if self.file_stem.is_empty() {
            "Main".to_string()
        } else {
            self.file_stem.clone()
        }
    }
}

/// Command a Build or Run action should type into the terminal.
pub fn task_command(facts: &ProjectFacts, build: bool) -> String {
    if facts.has_cargo {
        return if build { "cargo build" } else { "cargo run" }.into();
    }
    if facts.has_package_json {
        return if build {
            "npm run build"
        } else {
            "npm run dev"
        }
        .into();
    }
    if facts.has_go_mod {
        return if build { "go build ./..." } else { "go run ." }.into();
    }
    let quoted_file = facts.quoted_file();
    match facts.language.as_str() {
        "Python" => {
            if build {
                "python -m compileall .".into()
            } else {
                format!("python {quoted_file}")
            }
        }
        "C" => {
            if build {
                format!("gcc {quoted_file} -o {}", facts.executable())
            } else {
                facts.executable()
            }
        }
        "C++" => {
            if build {
                format!("g++ {quoted_file} -o {}", facts.executable())
            } else {
                facts.executable()
            }
        }
        "Java" => {
            if build {
                format!("javac {quoted_file}")
            } else {
                format!("java {}", facts.class_name())
            }
        }
        _ => format!(
            "echo 'No {} task configured for this project'",
            if build { "build" } else { "run" }
        ),
    }
}

/// Package formats TC-IDE can produce.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackageTarget {
    WindowsExecutable,
    PortableZip,
    Msix,
    Deb,
    AppImage,
    Dmg,
}

impl PackageTarget {
    pub const ALL: [PackageTarget; 6] = [
        PackageTarget::WindowsExecutable,
        PackageTarget::PortableZip,
        PackageTarget::Msix,
        PackageTarget::Deb,
        PackageTarget::AppImage,
        PackageTarget::Dmg,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PackageTarget::WindowsExecutable => "Windows .exe",
            PackageTarget::PortableZip => "Portable ZIP",
            PackageTarget::Msix => "MSIX",
            PackageTarget::Deb => "Debian .deb",
            PackageTarget::AppImage => "AppImage",
            PackageTarget::Dmg => "macOS .dmg",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            PackageTarget::WindowsExecutable => "exe",
            PackageTarget::PortableZip | PackageTarget::Msix | PackageTarget::Dmg => {
                if self == PackageTarget::Msix {
                    "msix"
                } else if self == PackageTarget::Dmg {
                    "dmg"
                } else {
                    "zip"
                }
            }
            PackageTarget::Deb => "deb",
            PackageTarget::AppImage => "AppImage",
        }
    }

    /// Whether the format has to be produced on its own operating system.
    pub fn native_system(self) -> Option<&'static str> {
        match self {
            PackageTarget::Deb | PackageTarget::AppImage => Some("linux"),
            PackageTarget::Dmg => Some("macos"),
            _ => Some("windows"),
        }
    }

    /// Artifact name inside the project package directory.
    pub fn artifact_name(self, project: &str) -> String {
        format!("{project}.{}", self.extension())
    }

    pub fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|target| *target == self)
            .unwrap_or(0)
    }

    pub fn from_index(index: usize) -> Self {
        Self::ALL[index.min(Self::ALL.len() - 1)]
    }
}

/// Progress a packaging command line has reached so far.
///
/// The panel binds its animation to these markers instead of a timer, so the
/// progress only moves when the build actually does something.
pub fn packaging_progress(output: &str) -> Option<f32> {
    const MARKERS: [(&str, f32); 12] = [
        ("New-Item", 0.08),
        ("Compiling", 0.35),
        ("go build", 0.4),
        ("gcc ", 0.4),
        ("g++ ", 0.4),
        ("PyInstaller", 0.45),
        ("javac ", 0.4),
        ("Finished", 0.62),
        ("Compress-Archive", 0.78),
        ("makeappx", 0.88),
        ("MakeAppx", 0.88),
        ("pack /d", 0.9),
    ];
    let mut progress: Option<f32> = None;
    for line in output.lines() {
        for (needle, value) in MARKERS {
            if line.contains(needle) {
                progress = Some(progress.map_or(value, |current: f32| current.max(value)));
            }
        }
    }
    progress
}

/// One produced artifact, or one that failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtifactReport {
    pub target: PackageTarget,
    pub path: String,
    pub bytes: u64,
    pub seconds: u64,
    pub ok: bool,
    /// Command that failed, when the artifact is missing.
    pub failed_command: Option<String>,
}

impl ArtifactReport {
    /// One line of the report written next to the artifacts.
    pub fn line(&self) -> String {
        if self.ok {
            format!(
                "{}: {} · {} · {}s",
                self.target.label(),
                self.path,
                format_bytes(self.bytes),
                self.seconds
            )
        } else {
            format!(
                "{}: FAILED after {}s · {}",
                self.target.label(),
                self.seconds,
                self.failed_command.as_deref().unwrap_or("unknown command")
            )
        }
    }
}

/// Human readable byte size.
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// Report file written into the package directory.
pub const PACKAGE_REPORT_FILE: &str = "BUILD-RESULT.txt";

/// Full report of one packaging run.
pub fn packaging_report(reports: &[ArtifactReport], project: &str) -> String {
    let mut text = format!("TC-IDE packaging report for {project}\n");
    for report in reports {
        text.push_str(&report.line());
        text.push('\n');
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferences_round_trip_through_the_settings_file() {
        let preferences = Preferences {
            light_theme: true,
            ui_chinese: true,
            code_zoom: 4,
            code_font_family: 2,
            project_language: 3,
            project_environment: 1,
            language_installed: vec![true, false, true],
            language_versions: vec![0, 2, 1],
            background_path: Some("/tmp/wall.png".into()),
            background_opacity: 0.5,
            background_offset_x: -0.25,
            background_offset_y: 0.25,
            background_zoom: 2.0,
            ai_endpoint: "https://example.test/v1".into(),
            ai_model: "example-model".into(),
        };

        let text = serialize_preferences(&preferences);
        assert_eq!(parse_preferences(&text), preferences);
        assert!(text.contains("theme=light"));
        assert!(!text.contains("api_key"));
    }

    #[test]
    fn preferences_normalize_and_fall_back() {
        let parsed = parse_preferences(
            "theme=light\ncode_zoom=99\nbackground_opacity=0\nbackground_zoom=0\nbackground_offset_x=-9\nai_endpoint=\nai_model=\n",
        );
        assert_eq!(parsed.code_zoom, 8);
        assert_eq!(parsed.background_opacity, DEFAULT_BACKGROUND_OPACITY);
        assert_eq!(parsed.background_zoom, 1.0);
        assert_eq!(parsed.background_offset_x, -1.0);
        assert_eq!(parsed.ai_endpoint, DEFAULT_AI_ENDPOINT);
        assert_eq!(parsed.ai_model, DEFAULT_AI_MODEL);

        let broken = parse_preferences("theme\nnonsense\n=value\nlanguage=zh-CN");
        assert!(broken.ui_chinese);
        assert!(!broken.light_theme);
    }

    #[test]
    fn linux_and_windows_font_keys_are_both_understood() {
        assert_eq!(parse_preferences("font_family=3\n").code_font_family, 3);
        assert_eq!(
            parse_preferences("code_font_family=4\n").code_font_family,
            4
        );
    }

    #[test]
    fn templates_cover_every_project_language() {
        for language in ["Rust", "Python", "C", "C++", "Java", "Go", "Web"] {
            let template = project_template("My App", language, "1.0", "None");
            assert!(!template.files.is_empty(), "{language} has no files");
            assert!(template.metadata.contains(&format!("language={language}")));
            assert!(template.metadata.contains("version=1.0"));
            assert!(template.metadata.contains("environment=None"));
            for file in &template.files {
                assert!(!file.path.starts_with('/'));
                assert!(!file.contents.is_empty());
            }
        }
        let rust = project_template("My App", "Rust", "stable", "None");
        assert!(rust.files.iter().any(|file| file.path == "src/main.rs"));
        assert!(rust
            .files
            .iter()
            .any(|file| file.path == "Cargo.toml" && file.contents.contains("my_app")));
        let python = project_template("x", "Python", "3.13", "venv");
        assert!(python.files.iter().any(|file| file.path == ".gitignore"));
    }

    #[test]
    fn identifiers_are_sanitized_for_package_names() {
        assert_eq!(sanitize_ident("My App"), "my_app");
        assert_eq!(sanitize_ident("  "), "tc_project");
        assert_eq!(sanitize_ident("3d-viewer"), "tc_3d_viewer");
        assert_eq!(sanitize_ident("a--b"), "a_b");
    }

    #[test]
    fn task_commands_follow_the_project_kind() {
        let base = ProjectFacts {
            has_cargo: true,
            language: "Rust".into(),
            file_name: "main.rs".into(),
            file_stem: "main".into(),
            windows: true,
            ..ProjectFacts::default()
        };
        assert_eq!(task_command(&base, true), "cargo build");
        assert_eq!(task_command(&base, false), "cargo run");

        let web = ProjectFacts {
            has_cargo: false,
            has_package_json: true,
            language: "TypeScript".into(),
            ..base.clone()
        };
        assert_eq!(task_command(&web, true), "npm run build");
        assert_eq!(task_command(&web, false), "npm run dev");

        let go = ProjectFacts {
            has_cargo: false,
            has_go_mod: true,
            language: "Go".into(),
            file_name: "main.go".into(),
            ..base.clone()
        };
        assert_eq!(task_command(&go, true), "go build ./...");
        assert_eq!(task_command(&go, false), "go run .");
    }

    #[test]
    fn c_and_java_commands_use_the_platform_shell() {
        let c = ProjectFacts {
            language: "C".into(),
            file_name: "main.c".into(),
            file_stem: "main".into(),
            windows: true,
            ..ProjectFacts::default()
        };
        assert_eq!(
            task_command(&c, true),
            "gcc \"main.c\" -o .\\tc-ide-build.exe"
        );
        assert_eq!(task_command(&c, false), ".\\tc-ide-build.exe");
        let unix = ProjectFacts {
            windows: false,
            ..c.clone()
        };
        assert_eq!(
            task_command(&unix, true),
            "gcc \"main.c\" -o ./tc-ide-build"
        );
        assert_eq!(task_command(&unix, false), "./tc-ide-build");

        let java = ProjectFacts {
            language: "Java".into(),
            file_name: "Main.java".into(),
            file_stem: "Main".into(),
            windows: true,
            ..ProjectFacts::default()
        };
        assert_eq!(task_command(&java, true), "javac \"Main.java\"");
        assert_eq!(task_command(&java, false), "java Main");
    }

    #[test]
    fn unknown_projects_report_a_missing_task_instead_of_silence() {
        let facts = ProjectFacts {
            language: "Markdown".into(),
            file_name: "README.md".into(),
            file_stem: "README".into(),
            windows: true,
            ..ProjectFacts::default()
        };
        let command = task_command(&facts, true);
        assert!(command.contains("No build task configured"));
        assert!(task_command(&facts, false).contains("No run task configured"));
    }

    #[test]
    fn package_targets_describe_their_format_and_system() {
        assert_eq!(PackageTarget::PortableZip.extension(), "zip");
        assert_eq!(PackageTarget::Msix.extension(), "msix");
        assert_eq!(PackageTarget::Deb.extension(), "deb");
        assert_eq!(PackageTarget::AppImage.extension(), "AppImage");
        assert_eq!(PackageTarget::Dmg.extension(), "dmg");
        assert_eq!(PackageTarget::Deb.native_system(), Some("linux"));
        assert_eq!(PackageTarget::Dmg.native_system(), Some("macos"));
        assert_eq!(PackageTarget::Msix.native_system(), Some("windows"));
        assert_eq!(PackageTarget::from_index(2), PackageTarget::Msix);
        assert_eq!(PackageTarget::from_index(99), PackageTarget::Dmg);
        assert_eq!(
            PackageTarget::ALL[PackageTarget::Msix.index()],
            PackageTarget::Msix
        );
        assert_eq!(
            PackageTarget::WindowsExecutable.artifact_name("demo"),
            "demo.exe"
        );
    }

    #[test]
    fn packaging_progress_follows_the_markers_in_the_output() {
        assert_eq!(packaging_progress("PS C:\\demo>"), None);
        let started = packaging_progress("New-Item -ItemType Directory -Force '.tc-ide-packages'");
        assert_eq!(started, Some(0.08));
        let compiling = packaging_progress("   Compiling tc-ide v0.2.0");
        assert_eq!(compiling, Some(0.35));
        let finished = packaging_progress("    Finished release [optimized] target(s)");
        assert_eq!(finished, Some(0.62));
        let packed =
            packaging_progress("   Compiling x\n    Finished\nMakeAppx pack /d stage /p out.msix");
        assert_eq!(packed, Some(0.9));
        // Progress never goes backwards when an earlier marker repeats.
        assert_eq!(
            packaging_progress("MakeAppx pack /d s /p o\n   Compiling again"),
            Some(0.9)
        );
    }

    #[test]
    fn artifact_reports_name_the_path_size_duration_or_failure() {
        let zip = ArtifactReport {
            target: PackageTarget::PortableZip,
            path: ".tc-ide-packages/demo.zip".into(),
            bytes: 2 * 1024 * 1024,
            seconds: 12,
            ok: true,
            failed_command: None,
        };
        assert_eq!(
            zip.line(),
            "Portable ZIP: .tc-ide-packages/demo.zip · 2.0 MB · 12s"
        );
        let failed = ArtifactReport {
            target: PackageTarget::Msix,
            path: ".tc-ide-packages/demo.msix".into(),
            bytes: 0,
            seconds: 3,
            ok: false,
            failed_command: Some("MakeAppx pack /d stage /p demo.msix".into()),
        };
        assert_eq!(
            failed.line(),
            "MSIX: FAILED after 3s · MakeAppx pack /d stage /p demo.msix"
        );
        let report = packaging_report(&[zip, failed.clone()], "demo");
        assert!(report.contains("demo.zip"));
        assert!(report.contains("FAILED"));
        assert!(report.starts_with("TC-IDE packaging report for demo"));
    }

    #[test]
    fn quoted_file_names_survive_spaces_and_quotes() {
        let facts = ProjectFacts {
            language: "Python".into(),
            file_name: "my \"app\".py".into(),
            file_stem: "my app".into(),
            windows: true,
            ..ProjectFacts::default()
        };
        assert_eq!(task_command(&facts, false), "python \"my \\\"app\\\".py\"");
    }
}
