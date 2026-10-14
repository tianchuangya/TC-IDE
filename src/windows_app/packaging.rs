//! Packaging: target selection, commands, progress and reports.

use super::*;

/// Icon written into every MSIX logo slot.
const MSIX_ICON: &[u8] = include_bytes!("../../assets/tc-ide-icon.png");

impl WindowsAppState {
    pub(super) fn start_project_environment_setup(&mut self) {
        let language = self.project_language_name();
        let environment = self.project_environments()[self
            .project_environment
            .min(self.project_environments().len() - 1)];
        let version_index = self
            .language_versions
            .get(self.project_language)
            .copied()
            .unwrap_or(0);
        let python_version = ["3.13", "3.12", "3.11"][version_index.min(2)];
        let command = match (language, environment) {
            (_, "None") => return,
            ("Python", "venv") => format!(
                "$py=(Get-Command py -ErrorAction SilentlyContinue); if($py){{py -{python_version} -m venv .venv}}else{{python -m venv .venv}}"
            ),
            ("Python", "uv") => format!("uv venv --python {python_version} .venv"),
            ("Python", "conda") => {
                format!("conda create -y -p .venv python={python_version}")
            }
            ("Python", "poetry") => format!("poetry env use {python_version}; poetry install"),
            ("Rust", "cargo nightly") => "rustup override set nightly".into(),
            ("Rust", "cargo stable") => "rustup override set stable".into(),
            ("Go", "Go modules") => "if(!(Test-Path go.mod)){go mod init tc-project}".into(),
            ("Web", manager) => format!("{manager} install"),
            _ => {
                self.status = format!(
                    "Project created; {} uses the system toolchain ({environment})",
                    language
                );
                return;
            }
        };
        let cache_root = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| self.project_root.clone())
            .join("TC-IDE")
            .join("runtime-cache")
            .join(language.to_ascii_lowercase())
            .join(if language == "Python" {
                python_version
            } else {
                environment
            });
        let _ = fs::create_dir_all(&cache_root);
        let _ = fs::write(
            cache_root.join("runtime.conf"),
            format!("language={language}\nversion={python_version}\nenvironment={environment}\n"),
        );
        self.terminal_open = true;
        self.ensure_terminal();
        if let Some(tab) = self.terminals.first() {
            let wrapped = format!(
                "$ErrorActionPreference='Stop'; Write-Host 'TC-IDE: preparing {language} {environment}'; try{{{command}; Write-Host 'TC-IDE: environment ready'}}catch{{Write-Host ('TC-IDE: environment setup failed: '+$_.Exception.Message)}}\r"
            );
            let _ = tab.session.write(&wrapped);
            self.active_terminal = 0;
            self.status = format!("Preparing {language} {environment} in the Build terminal");
        }
    }

    pub(super) fn package_settings_path(root: &std::path::Path) -> PathBuf {
        root.join(workspace_core::PACKAGE_TARGETS_FILE)
    }

    pub(super) fn load_package_targets(root: &std::path::Path) -> [bool; 3] {
        let text = fs::read_to_string(Self::package_settings_path(root)).unwrap_or_default();
        let stored = workspace_core::parse_package_targets(&text, 3);
        [stored[0], stored[1], stored[2]]
    }

    pub(super) fn save_package_targets(&self) {
        let _ = fs::write(
            Self::package_settings_path(&self.project_root),
            workspace_core::serialize_package_targets(&self.package_targets),
        );
    }

    pub(super) fn start_package(&mut self) {
        if !self.package_targets.iter().any(|selected| *selected) {
            self.status = "Select at least one package target".into();
            return;
        }
        self.terminal_open = true;
        self.ensure_terminal();
        let project = self
            .project_root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "TC-IDE-project".into())
            .replace(['\'', '"'], "");
        let file = self
            .file_path
            .as_ref()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "main".into())
            .replace('"', "");
        let stage = format!(".tc-ide-packages\\{project}");
        let is_rust = self.project_root.join("Cargo.toml").exists();
        let is_go = self.project_root.join("go.mod").exists();
        let language = editor_core::language_for_file(&self.file_name);
        let supports_windows_executable =
            is_rust || is_go || matches!(language, "C" | "C++" | "Python");
        if self.package_targets[2] && !supports_windows_executable {
            self.status = format!(
                "MSIX requires a Windows executable; {} packaging is not configured yet",
                language
            );
            return;
        }
        if self.package_targets[2] {
            let stage_path = self.project_root.join(".tc-ide-packages").join(&project);
            let assets = stage_path.join("Assets");
            let manifest = workspace_core::msix_manifest(&workspace_core::MsixPackage {
                project: project.clone(),
                identity: workspace_core::msix_identity(&project),
                display_name: workspace_core::msix_display_name(&project),
                version: concat!(env!("CARGO_PKG_VERSION"), ".0").to_string(),
            });
            let mut prepared = fs::create_dir_all(&assets)
                .and_then(|()| fs::write(stage_path.join("AppxManifest.xml"), manifest));
            for asset in workspace_core::MSIX_ASSET_FILES {
                if prepared.is_err() {
                    break;
                }
                prepared = fs::write(assets.join(asset), MSIX_ICON);
            }
            if let Err(error) = prepared {
                self.status = format!("Could not prepare MSIX layout: {error}");
                return;
            }
        }
        self.package_marker = format!("__TC_PACKAGE_DONE_{}__", self.started.elapsed().as_nanos());
        self.package_failure_marker = format!(
            "__TC_PACKAGE_FAILED_{}__",
            self.started.elapsed().as_nanos()
        );
        let fallback = self.task_command(true);
        let command = workspace_core::package_command(&workspace_core::PackageRequest {
            project,
            file,
            language: language.to_string(),
            stage,
            has_cargo: is_rust,
            has_go_mod: is_go,
            targets: self.package_targets,
            certificate: self
                .signing_certificate
                .as_ref()
                .map(|certificate| certificate.to_string_lossy().into_owned()),
            marker: self.package_marker.clone(),
            failure_marker: self.package_failure_marker.clone(),
            fallback,
        });
        if let Some(tab) = self.terminals.first() {
            let _ = tab.session.write(&format!("{command}\r"));
            self.active_terminal = 0;
            self.package_running = true;
            self.package_progress = 0.0;
            self.package_started = Some(Instant::now());
            self.package_open = false;
            self.status = "Packaging started in the Build terminal".into();
        }
    }

    /// Progress the packaging output has actually reached.
    pub(super) fn package_output_progress(&self) -> Option<f32> {
        let index = self.terminals.iter().position(|tab| tab.name == "Build")?;
        let output = self.terminals.get(index).and_then(|tab| {
            tab.session
                .view
                .lock()
                .ok()
                .map(|view| view.lines.join("\n"))
        })?;
        workspace_core::packaging_progress(&output)
    }

    /// Artifacts the last packaging run should have produced.
    pub(super) fn package_reports(
        &self,
        failed_command: Option<String>,
    ) -> Vec<workspace_core::ArtifactReport> {
        let project = self
            .project_root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "TC-IDE-project".into())
            .replace(['\'', '"'], "");
        let seconds = self
            .package_started
            .map(|started| started.elapsed().as_secs())
            .unwrap_or(0);
        let mut targets = Vec::new();
        if self.package_targets[0] {
            targets.push(workspace_core::PackageTarget::WindowsExecutable);
        }
        if self.package_targets[1] {
            targets.push(workspace_core::PackageTarget::PortableZip);
        }
        if self.package_targets[2] {
            targets.push(workspace_core::PackageTarget::Msix);
        }
        targets
            .into_iter()
            .map(|target| {
                let path = self
                    .project_root
                    .join(".tc-ide-packages")
                    .join(target.artifact_name(&project));
                let bytes = fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
                let ok = bytes > 0;
                workspace_core::ArtifactReport {
                    target,
                    path: path.to_string_lossy().into_owned(),
                    bytes,
                    seconds,
                    ok,
                    failed_command: if ok {
                        None
                    } else {
                        Some(
                            failed_command
                                .clone()
                                .unwrap_or_else(|| "the packaging command".to_string()),
                        )
                    },
                }
            })
            .collect()
    }

    /// Write BUILD-RESULT.txt next to the artifacts and report the outcome.
    pub(super) fn finish_package(&mut self, failed_command: Option<String>) {
        let reports = self.package_reports(failed_command);
        let project = self
            .project_root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "TC-IDE-project".into());
        let report = workspace_core::packaging_report(&reports, &project);
        let directory = self.project_root.join(".tc-ide-packages");
        let _ = fs::write(directory.join(workspace_core::PACKAGE_REPORT_FILE), &report);
        let failed = reports.iter().filter(|report| !report.ok).count();
        self.status = if failed == 0 {
            format!("Packaging finished: {}", reports[0].line())
        } else {
            format!("Packaging failed for {failed} target(s); see BUILD-RESULT.txt")
        };
    }
}
