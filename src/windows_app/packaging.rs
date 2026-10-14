//! Packaging: target selection, commands, progress and reports.

use super::*;

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
        root.join(".tc-ide-package-targets")
    }

    pub(super) fn load_package_targets(root: &std::path::Path) -> [bool; 3] {
        let mut targets = [false; 3];
        if let Ok(value) = fs::read_to_string(Self::package_settings_path(root)) {
            for (index, item) in value.trim().split(',').take(3).enumerate() {
                targets[index] = item == "1";
            }
        }
        targets
    }

    pub(super) fn save_package_targets(&self) {
        let value = self
            .package_targets
            .iter()
            .map(|selected| if *selected { "1" } else { "0" })
            .collect::<Vec<_>>()
            .join(",");
        let _ = fs::write(Self::package_settings_path(&self.project_root), value);
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
            let identity = project
                .chars()
                .map(|value| {
                    if value.is_ascii_alphanumeric() {
                        value
                    } else {
                        '.'
                    }
                })
                .collect::<String>();
            let identity = format!("TianChuangYa.{}", identity.trim_matches('.'));
            let display = project.replace(['&', '<', '>'], "");
            let manifest = format!(
                r#"<?xml version="1.0" encoding="utf-8"?>
<Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10" xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10" xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities" IgnorableNamespaces="uap rescap">
  <Identity Name="{identity}" Publisher="CN=TianChuangYa" Version="0.2.0.0" />
  <Properties><DisplayName>{display}</DisplayName><PublisherDisplayName>TianChuangYa</PublisherDisplayName><Logo>Assets\StoreLogo.png</Logo></Properties>
  <Resources><Resource Language="en-us" /></Resources>
  <Dependencies><TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.17763.0" MaxVersionTested="10.0.26100.0" /></Dependencies>
  <Applications><Application Id="TCIDE" Executable="{project}.exe" EntryPoint="Windows.FullTrustApplication"><uap:VisualElements DisplayName="{display}" Description="TC-IDE packaged project" BackgroundColor="transparent" Square150x150Logo="Assets\Square150x150Logo.png" Square44x44Logo="Assets\Square44x44Logo.png" /></Application></Applications>
  <Capabilities><rescap:Capability Name="runFullTrust" /></Capabilities>
</Package>
"#
            );
            let prepared = fs::create_dir_all(&assets)
                .and_then(|()| fs::write(stage_path.join("AppxManifest.xml"), manifest))
                .and_then(|()| {
                    fs::write(
                        assets.join("StoreLogo.png"),
                        include_bytes!("../../assets/tc-ide-icon.png"),
                    )
                })
                .and_then(|()| {
                    fs::write(
                        assets.join("Square150x150Logo.png"),
                        include_bytes!("../../assets/tc-ide-icon.png"),
                    )
                })
                .and_then(|()| {
                    fs::write(
                        assets.join("Square44x44Logo.png"),
                        include_bytes!("../../assets/tc-ide-icon.png"),
                    )
                });
            if let Err(error) = prepared {
                self.status = format!("Could not prepare MSIX layout: {error}");
                return;
            }
        }
        let build = if is_rust {
            format!(
                "cargo build --release; $a=Get-ChildItem .\\target\\release\\*.exe | Where-Object {{$_.DirectoryName -notmatch 'deps'}} | Select-Object -First 1; if($a){{Copy-Item $a.FullName '{stage}\\{project}.exe' -Force}}"
            )
        } else if is_go {
            format!("go build -o '{stage}\\{project}.exe' .")
        } else {
            match language {
                "C" => format!("gcc \"{file}\" -o '{stage}\\{project}.exe'"),
                "C++" => format!("g++ \"{file}\" -o '{stage}\\{project}.exe'"),
                "Python" => format!("python -m PyInstaller --noconfirm --onefile --name \"{project}\" \"{file}\" --distpath '{stage}' --workpath .tc-ide-packages\\work --specpath .tc-ide-packages\\spec"),
                "Java" => format!("javac \"{file}\"; jar --create --file '{stage}\\{project}.jar' *.class"),
                _ => self.task_command(true),
            }
        };
        let zip = if self.package_targets[1] {
            format!("; Compress-Archive -Path '{stage}\\*' -DestinationPath '.tc-ide-packages\\{project}.zip' -Force")
        } else {
            String::new()
        };
        let msix = if self.package_targets[2] {
            format!("; $makeappx=(Get-Command MakeAppx.exe -ErrorAction SilentlyContinue).Source; if(!$makeappx){{$makeappx=Get-ChildItem \"${{env:ProgramFiles(x86)}}\\Windows Kits\\10\\bin\\*\\x64\\makeappx.exe\" -ErrorAction SilentlyContinue | Sort-Object FullName -Descending | Select-Object -First 1 -ExpandProperty FullName}}; if(!$makeappx){{throw 'Windows SDK MakeAppx.exe is not installed'}}; & $makeappx pack /d '{stage}' /p '.tc-ide-packages\\{project}.msix' /o")
        } else {
            String::new()
        };
        let signing = match (&self.signing_certificate, self.package_targets[2]) {
            (Some(certificate), true) => format!(
                "; $signtool=(Get-Command signtool.exe -ErrorAction SilentlyContinue).Source; if(!$signtool){{$signtool=Get-ChildItem \"${{env:ProgramFiles(x86)}}\\Windows Kits\\10\\bin\\*\\x64\\signtool.exe\" -ErrorAction SilentlyContinue | Sort-Object FullName -Descending | Select-Object -First 1 -ExpandProperty FullName}}; if(!$signtool){{throw 'Windows SDK signtool.exe is not installed'}}; & $signtool sign /fd SHA256 /f '{}' '.tc-ide-packages\\{project}.msix'",
                certificate.to_string_lossy()
            ),
            _ => String::new(),
        };
        self.package_marker = format!("__TC_PACKAGE_DONE_{}__", self.started.elapsed().as_nanos());
        self.package_failure_marker = format!(
            "__TC_PACKAGE_FAILED_{}__",
            self.started.elapsed().as_nanos()
        );
        let command = format!(
            "$ErrorActionPreference='Stop'; try{{New-Item -ItemType Directory -Force '{stage}' | Out-Null; {build}{zip}{msix}{signing}; Write-Output '{}'}}catch{{Write-Host $_.Exception.Message; Write-Output '{}'}}",
            self.package_marker, self.package_failure_marker
        );
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

    /// Progress the packaging output has actually reached.    /// Progress the packaging output has actually reached.
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
