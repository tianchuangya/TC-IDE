#!/usr/bin/env bash
# Build the Windows backend on Linux and run its state layer check under Wine.
#
# The Windows backend is otherwise only exercised by CI, whose logs are not
# readable from here, so a failure there used to be a guessing game. LLD can
# link a Windows binary if it is given the mingw runtime, which can be fetched
# and unpacked without root, and Wine can run the result. That is enough to
# run the state layer check - 40 assertions about the Windows scene, the
# packaging commands and the palettes - on this host.
#
# One time set up:
#   mkdir -p /tmp/mingw && cd /tmp/mingw
#   apt-get download gcc-mingw-w64-x86-64-win32 gcc-mingw-w64-x86-64-win32-runtime \
#                    mingw-w64-x86-64-dev binutils-mingw-w64-x86-64
#   for f in *.deb; do dpkg-deb -x "$f" root/; done
# Then: tools/windows_selfcheck_wine.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MINGW="${MINGW_ROOT:-/tmp/mingw/root}"
WORK="${WIN_WORK:-/tmp/tc-ide-winbuild}"

# The runtime lives under /tmp/mingw/root when it was unpacked locally, and
# directly under the prefix when it came from the distribution packages.
if [ -d "$MINGW/usr/x86_64-w64-mingw32/lib" ]; then
    LIBDIR="$MINGW/usr/x86_64-w64-mingw32/lib"
elif [ -d "$MINGW/x86_64-w64-mingw32/lib" ]; then
    LIBDIR="$MINGW/x86_64-w64-mingw32/lib"
else
    echo "mingw runtime not found under $MINGW; see the comments at the top of this script" >&2
    exit 2
fi
GCCDIR="$(dirname "$(find "$MINGW" -name libgcc.a -path '*x86_64-w64-mingw32*' 2>/dev/null | head -1)")"
if [ -z "$GCCDIR" ] || [ ! -d "$GCCDIR" ]; then
    echo "mingw libgcc not found under $MINGW" >&2
    exit 2
fi
# The mingw tools have to be findable by name: rustc calls `dlltool` for the
# import libraries, and it does not take a path. Putting the prefix's bin
# directory on PATH is what makes a locally unpacked runtime work the same way
# as one installed by the distribution packages.
if [ -d "$MINGW/usr/bin" ]; then
    PATH="$MINGW/usr/bin:$PATH"
    export PATH
fi
command -v wine >/dev/null || { echo "wine is required" >&2; exit 2; }
command -v x86_64-w64-mingw32-dlltool >/dev/null || {
    echo "x86_64-w64-mingw32-dlltool not found; is the mingw bin directory on PATH?" >&2
    exit 2
}

# A library crate is built instead of the binary, because the binary would
# need a desktop; the check is reached through a small entry point.
rm -rf "$WORK"
mkdir -p "$WORK/src" "$WORK/.cargo"
cp -r "$ROOT/src/." "$WORK/src/"
cp -r "$ROOT/assets" "$WORK/"
cat > "$WORK/Cargo.toml" <<'TOML'
[package]
name = "tc-ide"
version = "0.3.0"
edition = "2021"

[lib]
name = "tc_ide"
path = "src/lib.rs"
TOML
# Keep every dependency table from the real manifest, not only the target
# sections: the shared cores use serde_json, and a scratch crate without it
# fails to compile for a reason that has nothing to do with the Windows port.
sed -n '/^\[dependencies\]/,$p' "$ROOT/Cargo.toml" >> "$WORK/Cargo.toml"

cat > "$WORK/.cargo/config.toml" <<TOML
[target.x86_64-pc-windows-gnu]
linker = "rust-lld"
rustflags = [
  "-C", "link-self-contained=yes",
  "-L", "native=$LIBDIR",
  "-L", "native=$GCCDIR",
]
TOML

# main.rs becomes a library, with the platform entry points removed and the
# Windows backend compiled even though the host is not Windows.
python3 - "$WORK" <<'PY'
import re, sys
work = sys.argv[1]
main = open(f"{work}/src/main.rs", encoding="utf-8").read()
main = re.sub(r'#\[cfg\(target_os = "[a-z]+"\)\]\nfn main\(\) \{[^}]*\}\n', '', main)
main = re.sub(r'#\[cfg\(not\(any\(target_os[^\n]*\n', '', main)
main = main.replace('compile_error!("TC-IDE currently supports Linux, Windows, and macOS targets only");', '')
main = main.replace('\nmod ', '\npub mod ')
main = main.replace('#[cfg(target_os = "windows")]\npub mod windows_app;', 'pub mod windows_app;')
open(f"{work}/src/lib.rs", "w", encoding="utf-8").write(main)
import os
os.remove(f"{work}/src/main.rs")

# a tiny binary that runs the check, so the exit code is the result
app = open(f"{work}/src/windows_app.rs", encoding="utf-8").read()
app = app.replace("mod selfcheck;", "pub mod selfcheck;", 1)
app += """

/// Run the state layer check without opening a window.
pub fn self_check_entry() -> i32 {
    selfcheck::self_check()
}
"""
open(f"{work}/src/windows_app.rs", "w", encoding="utf-8").write(app)
sc = open(f"{work}/src/windows_app/selfcheck.rs", encoding="utf-8").read()
sc = sc.replace("pub(super) fn self_check()", "pub fn self_check()", 1)
open(f"{work}/src/windows_app/selfcheck.rs", "w", encoding="utf-8").write(sc)
open(f"{work}/src/main.rs", "w", encoding="utf-8").write(
    "fn main() { std::process::exit(tc_ide::windows_app::self_check_entry()); }\n")
PY

cd "$WORK"
# The project's default cargo is 1.75, which is older than the windows crate
# needs, so the check runs on the stable toolchain the CI uses.
CARGO="cargo"
if command -v rustup >/dev/null; then
    CARGO="rustup run stable cargo"
fi
$CARGO build --target x86_64-pc-windows-gnu --bin tc-ide
wine target/x86_64-pc-windows-gnu/debug/tc-ide.exe
