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
}
