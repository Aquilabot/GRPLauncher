#!/usr/bin/env bash
# Builds the launcher for Windows from Linux, the portable zip, and (when makensis is available) the NSIS installer.
# Needs: rustup target x86_64-pc-windows-msvc, cargo-xwin (cargo install cargo-xwin), clang, lld, llvm-rc.
# Optional for GRPLauncher-Setup.exe: makensis (native or MAKENSIS="wine /path/to/makensis.exe").
#   ./build.sh            build dist/GRPLauncher.exe, dist/GRPLauncher.zip, dist/version.txt (and GRPLauncher-Setup.exe)
#   ./build.sh publish    the same, then upload the artifacts to the server (players' launchers replace themselves on next start)
set -euo pipefail
cd "$(dirname "$0")"
( cd src-tauri && cargo xwin build --release --target x86_64-pc-windows-msvc )
mkdir -p dist
cp src-tauri/target/x86_64-pc-windows-msvc/release/GRPLauncher.exe launcher.ini LEEME.txt LICENSE dist/
version="$(grep -oP 'pub const VERSION: u32 = \K[0-9]+' src-tauri/src/core.rs)"
semver="$(grep -oP '^version = "\K[^"]+' src-tauri/Cargo.toml)"
printf '%s %s\n' "$version" "$(sha256sum dist/GRPLauncher.exe | cut -d' ' -f1)" > dist/version.txt
( cd dist && rm -f GRPLauncher.zip && zip -q GRPLauncher.zip GRPLauncher.exe launcher.ini LEEME.txt LICENSE )

MAKENSIS_CMD="${MAKENSIS:-$(command -v makensis || true)}"
if [ -n "$MAKENSIS_CMD" ]; then
  wv2_cache="${XDG_CACHE_HOME:-$HOME/.cache}/grplauncher/MicrosoftEdgeWebview2Setup.exe"
  if [ ! -s "$wv2_cache" ]; then
    mkdir -p "$(dirname "$wv2_cache")"
    curl -fsSL "https://go.microsoft.com/fwlink/p/?LinkId=2124703" -o "$wv2_cache.tmp"
    mv "$wv2_cache.tmp" "$wv2_cache"
  fi
  cp "$wv2_cache" dist/MicrosoftEdgeWebview2Setup.exe
  # shellcheck disable=SC2086
  $MAKENSIS_CMD \
    -NOCD \
    -INPUTCHARSET UTF8 \
    -DAPP_VERSION="$semver" \
    -DDIST_DIR=dist \
    -DICON_FILE=src-tauri/icons/icon.ico \
    -DWEBVIEW2_BOOTSTRAPPER=MicrosoftEdgeWebview2Setup.exe \
    -DOUT_FILE=dist/GRPLauncher-Setup.exe \
    installer/grplauncher.nsi
  rm -f dist/MicrosoftEdgeWebview2Setup.exe
else
  echo "note: makensis not found; skipped GRPLauncher-Setup.exe (set MAKENSIS or build via release.yml)"
fi

ls -la dist
if [ "${1:-}" = publish ]; then
  files=(dist/GRPLauncher.exe dist/version.txt dist/GRPLauncher.zip)
  if [ -f dist/GRPLauncher-Setup.exe ]; then
    files+=(dist/GRPLauncher-Setup.exe)
  fi
  rsync -a "${files[@]}" "${GRP_HOST:-grp-server}:grp/launcher/"
  echo "published launcher $version"
fi
