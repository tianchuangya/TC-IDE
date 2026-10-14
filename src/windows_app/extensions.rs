//! Extensions: catalog, installation and tool probes.

use super::*;

impl WindowsAppState {
    /// Record an install once the terminal shows the matching checksum.
    pub(super) fn confirm_pending_extension(&mut self) {
        let Some(source) = self.pending_extension.clone() else {
            return;
        };
        let Some(index) = self
            .terminals
            .iter()
            .position(|tab| tab.name == "Output")
            .or_else(|| self.terminals.len().checked_sub(1))
        else {
            return;
        };
        let output = self
            .terminals
            .get(index)
            .and_then(|tab| {
                tab.session.view.lock().ok().map(|view| {
                    view.lines.join(
                        "
",
                    )
                })
            })
            .unwrap_or_default();
        let Some(digest) = extension_core::checksum_from_output(&output) else {
            return;
        };
        if !extension_core::verify_checksum(&digest, &source.sha256) {
            self.status = format!(
                "{} failed checksum verification; the download was not installed",
                source.name
            );
            self.pending_extension = None;
            return;
        }
        let root = extension_cache_root();
        let shell = if cfg!(windows) {
            extension_core::Shell::Windows
        } else {
            extension_core::Shell::Unix
        };
        let directory = extension_core::install_plan(&root.to_string_lossy(), &source, shell)
            .map(|plan| plan.directory)
            .unwrap_or_default();
        self.runtime_cache.record(extension_core::CacheEntry {
            id: source.id.clone(),
            version: source.version.clone(),
            sha256: digest,
            bytes: source.bytes,
            directory,
        });
        save_runtime_cache(&self.runtime_cache);
        self.pending_extension = None;
        self.status = format!(
            "{} {} verified and recorded in the runtime cache",
            source.name, source.version
        );
    }

    /// Catalog the project ships, when the user provides one.
    ///
    /// Real downloads need a real url and checksum, so the catalog is data the
    /// user supplies at `<project>/.tc-ide-catalog.conf` instead of a list this
    /// repository would have to invent.
    pub(super) fn project_catalog(&self) -> Result<Vec<extension_core::ExtensionSource>, String> {
        let path = self.project_root.join(".tc-ide-catalog.conf");
        let content = fs::read_to_string(&path).map_err(|_| {
            format!(
                "No extension catalog at {}; add one to install for real",
                path.display()
            )
        })?;
        extension_core::parse_catalog(&content)
    }

    /// Install the catalog entry for a language, when the project has one.
    pub(super) fn install_language_extension(&mut self, language: &str) {
        let catalog = match self.project_catalog() {
            Ok(catalog) => catalog,
            Err(message) => {
                self.status = message;
                return;
            }
        };
        let wanted = language.to_ascii_lowercase();
        let Some(source) = catalog.into_iter().find(|source| {
            source.name.to_ascii_lowercase() == wanted
                || source.id.to_ascii_lowercase().contains(&wanted)
        }) else {
            self.status = format!("No catalog entry matches {language}");
            return;
        };
        self.run_extension_install(&source);
    }

    /// Install an extension by planning the real download and unpack steps.
    pub(super) fn run_extension_install(&mut self, source: &extension_core::ExtensionSource) {
        let shell = if cfg!(windows) {
            extension_core::Shell::Windows
        } else {
            extension_core::Shell::Unix
        };
        let root = extension_cache_root();
        let plan = match extension_core::install_plan(&root.to_string_lossy(), source, shell) {
            Ok(plan) => plan,
            Err(error) => {
                self.status = error;
                return;
            }
        };
        if let Some(entry) = self.runtime_cache.reuse(&source.id, &source.version) {
            self.status = format!(
                "{} {} is already cached in {}",
                source.name, entry.version, entry.directory
            );
            return;
        }
        self.terminal_open = true;
        self.ensure_terminal();
        let Some(index) = self.terminals.len().checked_sub(1) else {
            return;
        };
        if let Some(tab) = self.terminals.get(index) {
            for command in &plan.commands {
                let _ = tab.session.write(&format!(
                    "{command}
"
                ));
            }
            self.active_terminal = index;
            self.pending_extension = Some(source.clone());
            self.status = format!(
                "Installing {} {} into {}",
                source.name, source.version, plan.directory
            );
        }
    }

    pub(super) fn probe_extension_tool(&mut self, label: &str, command: &str) {
        self.terminal_open = true;
        self.ensure_terminal();
        let index = 2.min(self.terminals.len().saturating_sub(1));
        let executable = command.split_whitespace().next().unwrap_or(command);
        let script = format!(
            "if(Get-Command '{executable}' -ErrorAction SilentlyContinue){{Write-Host 'TC-IDE: {label} detected'; {command}}}else{{Write-Host 'TC-IDE: {label} is not installed or not on PATH'}}\r"
        );
        if let Some(tab) = self.terminals.get(index) {
            let _ = tab.session.write(&script);
            self.active_terminal = index;
            self.status = format!("Checking {label} in the Output terminal");
        }
    }
}

/// User level runtime cache for extensions and runtimes.
pub(super) fn extension_cache_root() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("TC-IDE")
        .join("runtimes")
}

/// Manifest path of the runtime cache.
pub(super) fn runtime_cache_path() -> PathBuf {
    extension_cache_root().join("cache.conf")
}

pub(super) fn load_runtime_cache() -> extension_core::RuntimeCache {
    match fs::read_to_string(runtime_cache_path()) {
        Ok(content) => extension_core::RuntimeCache::parse(&content),
        Err(_) => extension_core::RuntimeCache::default(),
    }
}

pub(super) fn save_runtime_cache(cache: &extension_core::RuntimeCache) {
    let path = runtime_cache_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, cache.serialize());
}
