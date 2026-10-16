//! Shared extension catalog, download and runtime cache model.
//!
//! Installing an extension means: know what to fetch, fetch it through the
//! platform's own tool so the user can see and cancel it, verify the checksum,
//! unpack it into the user level runtime cache and record what is there. All of
//! that except the actual unpacking is data and rules, kept here so Linux
//! `cargo test` covers the catalog parser, the cache manifest, checksum checks
//! and the generated command lines.
#![allow(dead_code)] // The backends adopt the pieces they need as they migrate.

use std::fmt::Write as _;

/// What an extension provides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtensionKind {
    Language,
    Debugger,
    Formatter,
    Theme,
    Ai,
}

impl ExtensionKind {
    pub fn label(self) -> &'static str {
        match self {
            ExtensionKind::Language => "language",
            ExtensionKind::Debugger => "debugger",
            ExtensionKind::Formatter => "formatter",
            ExtensionKind::Theme => "theme",
            ExtensionKind::Ai => "ai",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "language" | "languages" => Some(ExtensionKind::Language),
            "debugger" | "debuggers" => Some(ExtensionKind::Debugger),
            "formatter" | "formatters" => Some(ExtensionKind::Formatter),
            "theme" | "themes" => Some(ExtensionKind::Theme),
            "ai" | "agents" => Some(ExtensionKind::Ai),
            _ => None,
        }
    }

    /// Minimum install state an extension must reach to be usable.
    pub fn is_runnable(self) -> bool {
        matches!(
            self,
            ExtensionKind::Language | ExtensionKind::Debugger | ExtensionKind::Formatter
        )
    }
}

/// One entry of an extension catalog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtensionSource {
    pub id: String,
    pub name: String,
    pub kind: ExtensionKind,
    pub version: String,
    pub url: String,
    pub sha256: String,
    pub bytes: u64,
    /// Executable the extension provides, when it provides one.
    pub binary: Option<String>,
}

/// A project-local plugin manifest. Plugins are deliberately data-first: the
/// manifest advertises commands, while TC-IDE runs them only after the user
/// clicks a visible command action in the Plugins panel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub commands: Vec<PluginCommand>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginCommand {
    pub id: String,
    pub label: String,
    pub command: String,
}

/// Parse a project plugin manifest from `[plugin]` and repeated `[command]`
/// sections. Unknown keys are ignored for forward compatibility.
pub fn parse_plugin_manifest(text: &str) -> Result<PluginManifest, String> {
    let mut plugin = PluginManifest {
        id: String::new(),
        name: String::new(),
        version: String::new(),
        description: String::new(),
        commands: Vec::new(),
    };
    let mut command: Option<PluginCommand> = None;
    let mut section = "";
    for (line_no, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "[plugin]" {
            if command.is_some() {
                return Err(format!(
                    "line {}: command section is incomplete",
                    line_no + 1
                ));
            }
            section = "plugin";
            continue;
        }
        if line == "[command]" {
            if let Some(value) = command.take() {
                finish_plugin_command(value, line_no, &mut plugin.commands)?;
            }
            section = "command";
            command = Some(PluginCommand {
                id: String::new(),
                label: String::new(),
                command: String::new(),
            });
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(format!("line {}: expected key=value", line_no + 1));
        };
        let value = value.trim();
        match section {
            "plugin" => match key.trim() {
                "id" => plugin.id = value.to_string(),
                "name" => plugin.name = value.to_string(),
                "version" => plugin.version = value.to_string(),
                "description" => plugin.description = value.to_string(),
                _ => {}
            },
            "command" => {
                let Some(current) = command.as_mut() else {
                    return Err(format!("line {}: command outside section", line_no + 1));
                };
                match key.trim() {
                    "id" => current.id = value.to_string(),
                    "label" => current.label = value.to_string(),
                    "command" => current.command = value.to_string(),
                    _ => {}
                }
            }
            _ => return Err(format!("line {}: value outside a section", line_no + 1)),
        }
    }
    if let Some(value) = command {
        finish_plugin_command(value, text.lines().count(), &mut plugin.commands)?;
    }
    if plugin.id.is_empty() || plugin.name.is_empty() || plugin.version.is_empty() {
        return Err("plugin requires id, name and version".into());
    }
    Ok(plugin)
}

fn finish_plugin_command(
    mut command: PluginCommand,
    line_no: usize,
    commands: &mut Vec<PluginCommand>,
) -> Result<(), String> {
    if command.id.is_empty() || command.command.is_empty() {
        return Err(format!(
            "line {}: command requires id and command",
            line_no + 1
        ));
    }
    if command.label.is_empty() {
        command.label = command.id.clone();
    }
    commands.push(command);
    Ok(())
}

impl ExtensionSource {
    /// Whether the catalog entry can be installed as it stands.
    pub fn is_installable(&self) -> bool {
        !self.id.is_empty()
            && !self.version.is_empty()
            && (self.url.starts_with("https://") || self.url.starts_with("http://localhost"))
            && self.sha256.len() == 64
            && self.sha256.chars().all(|value| value.is_ascii_hexdigit())
    }

    /// File name the archive is stored under, keeping the full extension so
    /// `tar.gz` archives stay recognisable.
    pub fn archive_name(&self) -> String {
        let file = self
            .url
            .rsplit('/')
            .next()
            .filter(|name| !name.is_empty())
            .unwrap_or("archive.zip");
        format!("{}-{}-{file}", self.id, self.version)
    }
}

/// Tools a language can use, and whether they are really installed.
///
/// The extension page reports this instead of a stored flag, so "installed"
/// always means "the executable is on PATH", and the same answer is used for the
/// language server, the formatter and the debugger.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LanguageTools {
    pub language: String,
    pub server: Option<String>,
    pub formatter: Option<String>,
    pub debugger: Option<String>,
    pub server_installed: bool,
    pub formatter_installed: bool,
    pub debugger_installed: bool,
}

impl LanguageTools {
    /// Whether at least one tool of this language is available.
    pub fn has_any(&self) -> bool {
        self.server_installed || self.formatter_installed || self.debugger_installed
    }

    /// Whether something is missing that the language would want.
    pub fn missing(&self) -> Vec<&str> {
        let mut missing = Vec::new();
        if self.server.is_some() && !self.server_installed {
            missing.push("language server");
        }
        if self.formatter.is_some() && !self.formatter_installed {
            missing.push("formatter");
        }
        if self.debugger.is_some() && !self.debugger_installed {
            missing.push("debugger");
        }
        missing
    }

    /// One line the panels can draw.
    pub fn summary(&self) -> String {
        let marks = [
            (self.server.as_deref(), self.server_installed, "LSP"),
            (self.formatter.as_deref(), self.formatter_installed, "fmt"),
            (self.debugger.as_deref(), self.debugger_installed, "dbg"),
        ];
        let parts = marks
            .iter()
            .filter_map(|(tool, installed, label)| {
                tool.map(|tool| {
                    format!(
                        "{label} {tool} {}",
                        if *installed { "ok" } else { "missing" }
                    )
                })
            })
            .collect::<Vec<_>>();
        if parts.is_empty() {
            "no external tooling".to_string()
        } else {
            parts.join(" · ")
        }
    }
}

/// Tool names TC-IDE looks for, per language.
pub fn tools_for_language(
    language: &str,
) -> (
    Option<&'static str>,
    Option<&'static str>,
    Option<&'static str>,
) {
    match language {
        "Rust" => (Some("rust-analyzer"), Some("rustfmt"), Some("lldb-dap")),
        "Python" => (
            Some("basedpyright-langserver"),
            Some("ruff"),
            Some("debugpy"),
        ),
        "C" | "C++" => (Some("clangd"), Some("clang-format"), Some("lldb-dap")),
        "Java" => (Some("jdtls"), None, None),
        "Go" => (Some("gopls"), Some("gofmt"), Some("dlv")),
        "TypeScript / JavaScript" | "Web" => {
            (Some("typescript-language-server"), Some("biome"), None)
        }
        _ => (None, None, None),
    }
}

/// Real installation state of a language's tooling.
///
/// `probe` is injected so the rules are testable without the tools present.
pub fn language_tools<F>(language: &str, probe: F) -> LanguageTools
where
    F: Fn(&str) -> bool,
{
    let (server, formatter, debugger) = tools_for_language(language);
    LanguageTools {
        language: language.to_string(),
        server: server.map(str::to_string),
        formatter: formatter.map(str::to_string),
        debugger: debugger.map(str::to_string),
        server_installed: server.map(&probe).unwrap_or(false),
        formatter_installed: formatter.map(&probe).unwrap_or(false),
        debugger_installed: debugger.map(&probe).unwrap_or(false),
    }
}

/// Parse a catalog written as `[extension]` sections of `key=value` lines.
pub fn parse_catalog(text: &str) -> Result<Vec<ExtensionSource>, String> {
    let mut sources = Vec::new();
    let mut current: Option<ExtensionSource> = None;
    let mut kind: Option<ExtensionKind> = None;
    for (number, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "[extension]" {
            if let Some(source) = current.take() {
                sources.push(finish_source(source, kind, number)?);
            }
            current = Some(ExtensionSource {
                id: String::new(),
                name: String::new(),
                kind: ExtensionKind::Language,
                version: String::new(),
                url: String::new(),
                sha256: String::new(),
                bytes: 0,
                binary: None,
            });
            kind = None;
            continue;
        }
        let Some(source) = current.as_mut() else {
            return Err(format!(
                "line {}: value outside an [extension] section",
                number + 1
            ));
        };
        let Some((key, value)) = line.split_once('=') else {
            return Err(format!("line {}: expected key=value", number + 1));
        };
        let value = value.trim();
        match key.trim() {
            "id" => source.id = value.to_string(),
            "name" => source.name = value.to_string(),
            "kind" => {
                kind = ExtensionKind::parse(value);
                if kind.is_none() {
                    return Err(format!("line {}: unknown kind {value}", number + 1));
                }
            }
            "version" => source.version = value.to_string(),
            "url" => source.url = value.to_string(),
            "sha256" => source.sha256 = value.to_ascii_lowercase(),
            "bytes" => source.bytes = value.parse().unwrap_or(0),
            "binary" if !value.is_empty() => source.binary = Some(value.to_string()),
            _ => {}
        }
    }
    if let Some(source) = current.take() {
        sources.push(finish_source(source, kind, text.lines().count())?);
    }
    Ok(sources)
}

fn finish_source(
    mut source: ExtensionSource,
    kind: Option<ExtensionKind>,
    number: usize,
) -> Result<ExtensionSource, String> {
    let Some(kind) = kind else {
        return Err(format!(
            "extension {} at line {} is missing a kind",
            if source.id.is_empty() {
                "?"
            } else {
                &source.id
            },
            number
        ));
    };
    source.kind = kind;
    if source.name.is_empty() {
        source.name = source.id.clone();
    }
    Ok(source)
}

/// What the runtime cache knows about one installed extension.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CacheEntry {
    pub id: String,
    pub version: String,
    pub sha256: String,
    pub bytes: u64,
    pub directory: String,
}

/// User level runtime cache manifest.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RuntimeCache {
    entries: Vec<CacheEntry>,
}

impl RuntimeCache {
    pub fn entries(&self) -> &[CacheEntry] {
        &self.entries
    }

    /// Record an installed extension, replacing a previous version.
    pub fn record(&mut self, entry: CacheEntry) {
        self.entries.retain(|existing| existing.id != entry.id);
        self.entries.push(entry);
        self.entries.sort_by(|left, right| left.id.cmp(&right.id));
    }

    /// A cached version that can be reused instead of downloading again.
    pub fn reuse(&self, id: &str, version: &str) -> Option<&CacheEntry> {
        self.entries
            .iter()
            .find(|entry| entry.id == id && entry.version == version)
    }

    /// Cached versions of an extension, newest first by insertion order.
    pub fn versions(&self, id: &str) -> Vec<&CacheEntry> {
        self.entries.iter().filter(|entry| entry.id == id).collect()
    }

    pub fn remove(&mut self, id: &str) -> Option<CacheEntry> {
        let position = self.entries.iter().position(|entry| entry.id == id)?;
        Some(self.entries.remove(position))
    }

    pub fn total_bytes(&self) -> u64 {
        self.entries.iter().map(|entry| entry.bytes).sum()
    }

    /// Drop everything at or above a size budget, oldest entries first.
    pub fn prune_to(&mut self, budget_bytes: u64) -> Vec<CacheEntry> {
        let mut removed = Vec::new();
        while self.total_bytes() > budget_bytes && !self.entries.is_empty() {
            removed.push(self.entries.remove(0));
        }
        removed
    }

    pub fn serialize(&self) -> String {
        let mut text = String::new();
        for entry in &self.entries {
            let _ = writeln!(
                text,
                "[cached]\nid={}\nversion={}\nsha256={}\nbytes={}\ndirectory={}",
                entry.id, entry.version, entry.sha256, entry.bytes, entry.directory
            );
        }
        text
    }

    pub fn parse(text: &str) -> RuntimeCache {
        let mut cache = RuntimeCache::default();
        let mut current: Option<CacheEntry> = None;
        for line in text.lines() {
            let line = line.trim();
            if line == "[cached]" {
                if let Some(entry) = current.take() {
                    cache.record(entry);
                }
                current = Some(CacheEntry {
                    id: String::new(),
                    version: String::new(),
                    sha256: String::new(),
                    bytes: 0,
                    directory: String::new(),
                });
                continue;
            }
            let Some(entry) = current.as_mut() else {
                continue;
            };
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key.trim() {
                "id" => entry.id = value.trim().to_string(),
                "version" => entry.version = value.trim().to_string(),
                "sha256" => entry.sha256 = value.trim().to_ascii_lowercase(),
                "bytes" => entry.bytes = value.trim().parse().unwrap_or(0),
                "directory" => entry.directory = value.trim().to_string(),
                _ => {}
            }
        }
        if let Some(entry) = current.take() {
            if !entry.id.is_empty() {
                cache.record(entry);
            }
        }
        cache
    }
}

/// Compare a computed checksum with the expected one.
pub fn verify_checksum(actual: &str, expected: &str) -> bool {
    let actual = actual.trim().to_ascii_lowercase();
    let expected = expected.trim().to_ascii_lowercase();
    actual.len() == 64 && actual == expected
}

/// Extract the hex digest from `sha256sum`, `shasum` or `certutil` output.
pub fn checksum_from_output(output: &str) -> Option<String> {
    for token in output.split(|value: char| !value.is_ascii_hexdigit()) {
        if token.len() == 64 && token.chars().all(|value| value.is_ascii_hexdigit()) {
            return Some(token.to_ascii_lowercase());
        }
    }
    None
}

/// Platform a command line is generated for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shell {
    Windows,
    Unix,
}

/// Command that downloads an archive into the cache.
pub fn download_command(source: &ExtensionSource, destination: &str, shell: Shell) -> String {
    match shell {
        // curl.exe ships with Windows 10 and later.
        Shell::Windows => format!(
            "curl.exe -L --fail --max-time 600 -o \"{destination}\" \"{}\"",
            source.url
        ),
        Shell::Unix => format!(
            "curl -L --fail --max-time 600 -o \"{destination}\" \"{}\"",
            source.url
        ),
    }
}

/// Command that prints the checksum of a downloaded archive.
pub fn checksum_command(path: &str, shell: Shell) -> String {
    match shell {
        Shell::Windows => format!("certutil -hashfile \"{path}\" SHA256"),
        Shell::Unix => format!("sha256sum \"{path}\""),
    }
}

/// Command that unpacks an archive into the cache directory.
pub fn unpack_command(archive: &str, directory: &str, shell: Shell) -> String {
    match shell {
        Shell::Windows => format!(
            "powershell -NoProfile -Command \"Expand-Archive -Force -LiteralPath '{archive}' -DestinationPath '{directory}'\""
        ),
        Shell::Unix => format!("unzip -o \"{archive}\" -d \"{directory}\""),
    }
}

/// Where an extension of a version lives inside the cache root.
pub fn install_directory(root: &str, source: &ExtensionSource) -> String {
    format!("{root}/{}-{}", source.id, source.version)
}

/// Steps a real install performs, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstallPlan {
    pub directory: String,
    pub archive: String,
    pub commands: Vec<String>,
    pub binary: Option<String>,
}

/// Plan an install: download, verify, unpack, then report.
pub fn install_plan(
    root: &str,
    source: &ExtensionSource,
    shell: Shell,
) -> Result<InstallPlan, String> {
    if !source.is_installable() {
        return Err(format!(
            "{} is not installable: the catalog needs an https url and a sha256 of 64 hex digits",
            source.id
        ));
    }
    let directory = install_directory(root, source);
    let archive = format!("{root}/{}", source.archive_name());
    Ok(InstallPlan {
        commands: vec![
            download_command(source, &archive, shell),
            checksum_command(&archive, shell),
            unpack_command(&archive, &directory, shell),
        ],
        binary: source.binary.clone(),
        directory,
        archive,
    })
}

/// Whether `candidate` is a newer version than `current`.
///
/// Versions are compared part by part, so `1.2.10` is newer than `1.2.9`, which a
/// string comparison gets wrong. A version with a suffix (`1.0.0-beta`) is older
/// than the plain release of the same numbers, and anything that is not made of
/// numbers at all falls back to "a different string is a newer one", which is
/// what a user of a hand written catalog expects.
pub fn version_is_newer(candidate: &str, current: &str) -> bool {
    let candidate = candidate.trim();
    let current = current.trim();
    if candidate == current {
        return false;
    }
    let (candidate_numbers, candidate_suffix) = split_version(candidate);
    let (current_numbers, current_suffix) = split_version(current);
    if candidate_numbers.is_empty() || current_numbers.is_empty() {
        return true;
    }
    for index in 0..candidate_numbers.len().max(current_numbers.len()) {
        let left = candidate_numbers.get(index).copied().unwrap_or(0);
        let right = current_numbers.get(index).copied().unwrap_or(0);
        if left != right {
            return left > right;
        }
    }
    // Same numbers: a plain release beats a prerelease of the same version.
    match (candidate_suffix.is_empty(), current_suffix.is_empty()) {
        (true, false) => true,
        (false, true) => false,
        _ => candidate_suffix > current_suffix,
    }
}

/// Numbers and suffix of a dotted version.
fn split_version(version: &str) -> (Vec<u64>, String) {
    let mut numbers = Vec::new();
    let mut suffix = String::new();
    for (index, part) in version.split(['.', '-', '+']).enumerate() {
        match part.parse::<u64>() {
            Ok(number) => numbers.push(number),
            Err(_) => {
                suffix = version
                    .split(['-', '+'])
                    .nth(1)
                    .unwrap_or_default()
                    .to_string();
                let _ = index;
                break;
            }
        }
    }
    (numbers, suffix)
}

/// What is on offer for an extension that is already installed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateState {
    /// The catalog has nothing newer.
    UpToDate,
    /// The catalog has a newer version.
    Upgrade,
    /// The extension is not in the catalog any more.
    NotInCatalog,
}

/// One extension of the runtime cache, and what the catalog says about it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdatePlan {
    pub id: String,
    pub name: String,
    pub installed: String,
    /// Version the catalog offers, when the catalog knows the extension.
    pub available: Option<String>,
    pub state: UpdateState,
}

impl UpdatePlan {
    /// One line for the extensions panel.
    pub fn describe(&self) -> String {
        match (self.state, self.available.as_deref()) {
            (UpdateState::Upgrade, Some(available)) => {
                format!("{} {} → {available} available", self.name, self.installed)
            }
            (UpdateState::UpToDate, _) => {
                format!("{} {} is up to date", self.name, self.installed)
            }
            (UpdateState::Upgrade, None) => {
                format!("{} {} can be upgraded", self.name, self.installed)
            }
            (UpdateState::NotInCatalog, _) => format!(
                "{} {} is installed; the catalog no longer lists it",
                self.name, self.installed
            ),
        }
    }
}

/// What the user can do with every installed extension.
///
/// The cache is the record of what is installed, and the catalog (which the user
/// supplies) is the record of what could be installed, so an upgrade is simply a
/// catalog entry whose version is newer than the cached one.
pub fn update_plan(cache: &RuntimeCache, catalog: &[ExtensionSource]) -> Vec<UpdatePlan> {
    let mut plans = cache
        .entries()
        .iter()
        .map(|entry| {
            let source = catalog.iter().find(|source| source.id == entry.id);
            let state = match source {
                Some(source) => {
                    if version_is_newer(&source.version, &entry.version) {
                        UpdateState::Upgrade
                    } else {
                        UpdateState::UpToDate
                    }
                }
                None => UpdateState::NotInCatalog,
            };
            UpdatePlan {
                id: entry.id.clone(),
                name: source
                    .map(|source| source.name.clone())
                    .unwrap_or_else(|| entry.id.clone()),
                installed: entry.version.clone(),
                available: source.map(|source| source.version.clone()),
                state,
            }
        })
        .collect::<Vec<_>>();
    plans.sort_by(|left, right| left.id.cmp(&right.id));
    plans
}

/// Steps an uninstall performs.
pub fn uninstall_commands(root: &str, entry: &CacheEntry, shell: Shell) -> Vec<String> {
    let directory = if entry.directory.is_empty() {
        format!("{root}/{}", entry.id)
    } else {
        entry.directory.clone()
    };
    match shell {
        Shell::Windows => vec![format!(
            "powershell -NoProfile -Command \"Remove-Item -Recurse -Force -LiteralPath '{directory}'\""
        )],
        Shell::Unix => vec![format!("rm -rf \"{directory}\"")],
    }
}

/// Parse the ar/zip layout summary needed for the artifact report.
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

#[cfg(test)]
mod tests {
    use super::*;

    const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn catalog_text() -> String {
        format!(
            "# catalog\n[extension]\nid=rust-analyzer\nname=Rust Analyzer\nkind=language\nversion=2026.10.1\nurl=https://example.test/rust-analyzer.zip\nsha256={DIGEST}\nbytes=1048576\nbinary=rust-analyzer\n\n[extension]\nid=gdb\nname=GDB\nkind=debugger\nversion=15.2\nurl=https://example.test/gdb.tar.gz\nsha256={DIGEST}\nbinary=gdb\n"
        )
    }

    #[test]
    fn catalog_sections_are_parsed() {
        let sources = parse_catalog(&catalog_text()).expect("catalog");
        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].id, "rust-analyzer");
        assert_eq!(sources[0].kind, ExtensionKind::Language);
        assert_eq!(sources[0].binary.as_deref(), Some("rust-analyzer"));
        assert!(sources[0].is_installable());
        assert_eq!(
            sources[0].archive_name(),
            "rust-analyzer-2026.10.1-rust-analyzer.zip"
        );
        assert_eq!(sources[1].archive_name(), "gdb-15.2-gdb.tar.gz");
        assert_eq!(sources[1].name, "GDB");
        assert!(sources[1].kind.is_runnable());
        assert!(!ExtensionKind::Theme.is_runnable());
    }

    #[test]
    fn catalog_rejects_broken_entries() {
        assert!(parse_catalog("id=x\n").is_err());
        assert!(parse_catalog("[extension]\nid=x\nkind=unknown\n").is_err());
        assert!(parse_catalog("[extension]\nid=x\nname=X\n").is_err());

        let mut source = parse_catalog(&catalog_text()).unwrap().remove(0);
        source.sha256 = "abc".into();
        assert!(!source.is_installable());
        source.sha256 = DIGEST.into();
        source.url = "http://example.test/x.zip".into();
        assert!(!source.is_installable());
        assert!(install_plan("/cache", &source, Shell::Unix).is_err());
    }

    #[test]
    fn language_tooling_reports_what_is_really_installed() {
        let installed = |tool: &str| matches!(tool, "rust-analyzer" | "rustfmt");
        let rust = language_tools("Rust", installed);
        assert_eq!(rust.server.as_deref(), Some("rust-analyzer"));
        assert!(rust.server_installed);
        assert!(rust.formatter_installed);
        assert!(!rust.debugger_installed);
        assert!(rust.has_any());
        assert_eq!(rust.missing(), ["debugger"]);
        assert!(rust.summary().contains("LSP rust-analyzer ok"));
        assert!(rust.summary().contains("dbg lldb-dap missing"));

        let nothing = |_: &str| false;
        let python = language_tools("Python", nothing);
        assert!(!python.has_any());
        assert_eq!(python.missing().len(), 3);

        let unknown = language_tools("Markdown", |_| true);
        assert!(unknown.server.is_none());
        assert!(!unknown.has_any());
        assert_eq!(unknown.summary(), "no external tooling");

        // Java has a server but no formatter or debugger configured.
        let java = language_tools("Java", nothing);
        assert_eq!(java.formatter, None);
        assert_eq!(java.missing(), ["language server"]);
    }

    #[test]
    fn cache_records_versions_and_reuses_them() {
        let mut cache = RuntimeCache::default();
        cache.record(CacheEntry {
            id: "gdb".into(),
            version: "15.1".into(),
            sha256: DIGEST.into(),
            bytes: 1000,
            directory: "/cache/gdb-15.1".into(),
        });
        cache.record(CacheEntry {
            id: "gdb".into(),
            version: "15.2".into(),
            sha256: DIGEST.into(),
            bytes: 2000,
            directory: "/cache/gdb-15.2".into(),
        });
        cache.record(CacheEntry {
            id: "ruff".into(),
            version: "0.6".into(),
            sha256: DIGEST.into(),
            bytes: 500,
            directory: "/cache/ruff-0.6".into(),
        });
        assert_eq!(cache.entries().len(), 2);
        assert_eq!(
            cache.reuse("gdb", "15.2").map(|entry| entry.bytes),
            Some(2000)
        );
        assert!(cache.reuse("gdb", "14.0").is_none());
        assert_eq!(cache.total_bytes(), 2500);
        assert_eq!(cache.versions("gdb").len(), 1);

        let text = cache.serialize();
        let restored = RuntimeCache::parse(&text);
        assert_eq!(restored, cache);

        let removed = cache.prune_to(1500);
        assert_eq!(removed.len(), 1);
        assert!(cache.total_bytes() <= 1500);
        assert!(cache.remove("ruff").is_some());
        assert!(cache.remove("ruff").is_none());
    }

    #[test]
    fn checksums_are_verified_case_insensitively() {
        assert!(verify_checksum(DIGEST, &DIGEST.to_uppercase()));
        assert!(!verify_checksum("abc", DIGEST));
        assert!(!verify_checksum(&DIGEST.replace('0', "1"), DIGEST));
        assert_eq!(
            checksum_from_output(&format!("{DIGEST}  /cache/rust-analyzer.zip")).as_deref(),
            Some(DIGEST)
        );
        assert_eq!(
            checksum_from_output(&format!("SHA256 hash of file:\n{DIGEST}\nCertUtil done"))
                .as_deref(),
            Some(DIGEST)
        );
        assert_eq!(checksum_from_output("no digest here"), None);
    }

    #[test]
    fn install_and_uninstall_commands_match_the_platform() {
        let sources = parse_catalog(&catalog_text()).unwrap();
        let plan = install_plan(
            "/home/user/.cache/tc-ide/extensions",
            &sources[0],
            Shell::Unix,
        )
        .expect("plan");
        assert!(plan.directory.ends_with("rust-analyzer-2026.10.1"));
        assert_eq!(plan.commands.len(), 3);
        assert!(plan.commands[0].starts_with("curl -L --fail"));
        assert!(plan.commands[1].starts_with("sha256sum"));
        assert!(plan.commands[2].starts_with("unzip -o"));

        let windows = install_plan("C:/cache", &sources[0], Shell::Windows).expect("plan");
        assert!(windows.commands[0].starts_with("curl.exe -L --fail"));
        assert!(windows.commands[1].starts_with("certutil -hashfile"));
        assert!(windows.commands[2].contains("Expand-Archive"));

        let entry = CacheEntry {
            id: "gdb".into(),
            version: "15.2".into(),
            sha256: DIGEST.into(),
            bytes: 10,
            directory: "/cache/gdb-15.2".into(),
        };
        let unix = uninstall_commands("/cache", &entry, Shell::Unix);
        assert_eq!(unix, vec!["rm -rf \"/cache/gdb-15.2\"".to_string()]);
        let windows = uninstall_commands("/cache", &entry, Shell::Windows);
        assert!(windows[0].contains("Remove-Item -Recurse -Force"));
    }

    #[test]
    fn byte_sizes_are_readable() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(999), "999 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1024 * 1024 * 3 / 2), "1.5 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024 * 2), "2.0 GB");
    }

    fn source_of(id: &str, name: &str, version: &str) -> ExtensionSource {
        ExtensionSource {
            id: id.into(),
            name: name.into(),
            version: version.into(),
            url: format!("https://example.invalid/{id}-{version}.zip"),
            sha256: "a".repeat(64),
            bytes: 1024,
            kind: ExtensionKind::Language,
            binary: None,
        }
    }

    #[test]
    fn versions_compare_number_by_number() {
        assert!(version_is_newer("1.2.10", "1.2.9"));
        assert!(!version_is_newer("1.2.9", "1.2.10"));
        assert!(version_is_newer("2.0", "1.9.9"));
        assert!(!version_is_newer("2.0.0", "2.0"));
        assert!(!version_is_newer("2.0", "2.0.0"));
        assert!(version_is_newer("1.0.1", "1.0"));
        // A prerelease is older than the release of the same numbers.
        assert!(!version_is_newer("1.0.0-beta", "1.0.0"));
        assert!(version_is_newer("1.0.0", "1.0.0-beta"));
        assert!(version_is_newer("1.0.0-rc2", "1.0.0-beta"));
        // Hand written versions fall back to "different means newer".
        assert!(version_is_newer("nightly", "stable"));
        assert!(!version_is_newer("stable", "stable"));
        assert!(version_is_newer(" 1.2 ", "1.1"));
    }

    #[test]
    fn the_update_plan_compares_the_cache_with_the_catalog() {
        let mut cache = RuntimeCache::default();
        let entry = |id: &str, version: &str| CacheEntry {
            id: id.into(),
            version: version.into(),
            sha256: "b".repeat(64),
            bytes: 10,
            directory: format!("/cache/{id}"),
        };
        cache.record(entry("rust-analyzer", "1.2.9"));
        cache.record(entry("clangd", "18.1.3"));
        cache.record(entry("gone", "0.1.0"));

        let catalog = vec![
            source_of("rust-analyzer", "rust-analyzer", "1.2.10"),
            source_of("clangd", "clangd", "18.1.3"),
        ];
        let plan = update_plan(&cache, &catalog);
        assert_eq!(plan.len(), 3);
        let by_id = |id: &str| {
            plan.iter()
                .find(|entry| entry.id == id)
                .expect("a plan entry")
        };
        assert_eq!(by_id("rust-analyzer").state, UpdateState::Upgrade);
        assert_eq!(by_id("rust-analyzer").available.as_deref(), Some("1.2.10"));
        assert!(by_id("rust-analyzer")
            .describe()
            .contains("1.2.9 → 1.2.10 available"));
        assert_eq!(by_id("clangd").state, UpdateState::UpToDate);
        assert!(by_id("clangd").describe().contains("up to date"));
        assert_eq!(by_id("gone").state, UpdateState::NotInCatalog);
        assert!(by_id("gone").describe().contains("no longer lists it"));

        // No catalog at all means nothing to upgrade to.
        let empty = update_plan(&cache, &[]);
        assert!(empty
            .iter()
            .all(|entry| entry.state == UpdateState::NotInCatalog));
        assert!(update_plan(&RuntimeCache::default(), &catalog).is_empty());

        // Uninstalling removes the entry and its plan.
        let removed = cache.remove("gone").expect("the entry");
        assert_eq!(removed.directory, "/cache/gone");
        assert!(!update_plan(&cache, &catalog)
            .iter()
            .any(|entry| entry.id == "gone"));
    }

    #[test]
    fn a_catalog_entry_is_planned_verified_and_removable() {
        // What the panel does with one entry: plan the install, check what it
        // downloaded, and be able to take it back out.
        let source = ExtensionSource {
            id: "rust-tools".into(),
            name: "Rust".into(),
            kind: ExtensionKind::Language,
            version: "1.2.0".into(),
            url: "https://example.invalid/rust-tools.tar.gz".into(),
            sha256: "a".repeat(64),
            bytes: 1024,
            binary: Some("rust-analyzer".into()),
        };
        assert!(source.is_installable(), "a url and a checksum is enough");

        let plan = install_plan("/cache", &source, Shell::Unix).expect("a plan");
        assert!(plan.directory.contains("rust-tools"), "{}", plan.directory);
        assert!(!plan.commands.is_empty(), "the download is a real command");
        // The commands name the url, so the terminal shows what was fetched,
        // and the plan prints the checksum that was expected beside the one
        // that arrived: the comparison is left to a caller that can read both.
        let joined = plan.commands.join("\n");
        assert!(joined.contains(&source.url), "{joined}");
        assert!(
            joined.contains("sha256sum") || joined.contains("shasum"),
            "the archive is hashed: {joined}"
        );
        // The digest the plan prints is the one the manifest expects.
        let printed = checksum_from_output(&format!("{}\n  {}/x\n", source.sha256, plan.archive));
        assert_eq!(printed.as_deref(), Some(source.sha256.as_str()));
        assert!(verify_checksum(
            printed.as_deref().unwrap_or(""),
            &source.sha256
        ));

        // A checksum that does not match is refused.
        let digest = "b".repeat(64);
        assert!(!verify_checksum(&digest, &source.sha256));
        assert!(verify_checksum(&source.sha256, &source.sha256));

        // Removing it produces commands too, rather than only forgetting it.
        let entry = CacheEntry {
            id: source.id.clone(),
            version: source.version.clone(),
            sha256: source.sha256.clone(),
            bytes: source.bytes,
            directory: plan.directory.clone(),
        };
        assert!(!uninstall_commands("/cache", &entry, Shell::Unix).is_empty());
    }

    #[test]
    fn an_entry_without_a_url_or_a_checksum_is_refused() {
        // The project ships no invented urls or checksums, so an entry that
        // lacks them is a configuration mistake that has to be reported.
        let mut source = ExtensionSource {
            id: "x".into(),
            name: "X".into(),
            kind: ExtensionKind::Language,
            version: "1".into(),
            url: "https://example.invalid/x.tgz".into(),
            sha256: "a".repeat(64),
            bytes: 1,
            binary: None,
        };
        assert!(install_plan("/cache", &source, Shell::Unix).is_ok());

        source.sha256 = "short".into();
        assert!(!source.is_installable());
        assert!(install_plan("/cache", &source, Shell::Unix).is_err());

        source.sha256 = "a".repeat(64);
        source.url = "http://plain.example.invalid/x.tgz".into();
        assert!(!source.is_installable(), "a download has to be https");
        assert!(install_plan("/cache", &source, Shell::Unix).is_err());
    }

    #[test]
    fn project_plugin_manifests_register_commands() {
        let manifest = parse_plugin_manifest(
            "[plugin]\nid=preview\nname=Preview Tools\nversion=0.1.0\ndescription=fast preview\n\n[command]\nid=serve\nlabel=Start preview\ncommand=npm run dev\n\n[command]\nid=check\ncommand=cargo check\n",
        )
        .expect("manifest");
        assert_eq!(manifest.id, "preview");
        assert_eq!(manifest.commands.len(), 2);
        assert_eq!(manifest.commands[1].label, "check");
        assert!(parse_plugin_manifest("[plugin]\nid=broken\n").is_err());
    }
}
