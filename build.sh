#!/usr/bin/env bash
# Builds the launcher for Windows and Linux, the portable zip, and (when makensis is available) the NSIS installer.
# Needs: rustup target x86_64-pc-windows-msvc, cargo-xwin (cargo install cargo-xwin), clang, lld, llvm-rc.
# Optional for GRPLauncher-Setup.exe: makensis (native or MAKENSIS="wine /path/to/makensis.exe").
#   ./build.sh            build Windows (dist/GRPLauncher.exe, dist/GRPLauncher.zip, dist/version.txt, setup) and Linux (dist/GRPLauncher-linux, dist/version-linux.txt, dist/linux/GRPLauncher)
# Needs the publisher's signing key (tools/sign.sh) to write the version lists.
#   ./build.sh publish    the same, then upload the artifacts to the server (players' launchers replace themselves on next start)
set -euo pipefail
cd "$(dirname "$0")"
( cd src-tauri && cargo xwin build --release --target x86_64-pc-windows-msvc )
( cd src-tauri && cargo build --release )
mkdir -p dist dist/linux
cp src-tauri/target/x86_64-pc-windows-msvc/release/GRPLauncher.exe launcher.ini LEEME.txt LICENSE dist/
cp src-tauri/target/release/GRPLauncher dist/GRPLauncher-linux
cp src-tauri/target/release/GRPLauncher dist/linux/GRPLauncher
if [ ! -f dist/linux/launcher.ini ]; then
  cp launcher.ini dist/linux/launcher.ini
fi
version="$(grep -oP 'pub const VERSION: u32 = \K[0-9]+' src-tauri/src/core.rs)"
semver="$(grep -oP '^version = "\K[^"]+' src-tauri/Cargo.toml)"
printf '%s %s\n' "$version" "$(sha256sum dist/GRPLauncher.exe | cut -d' ' -f1)" > dist/version.txt
printf '%s %s\n' "$version" "$(sha256sum dist/GRPLauncher-linux | cut -d' ' -f1)" > dist/version-linux.txt
# Launchers from version 25 take an update only from a version list with the publisher's signature (tools/sign.sh).
tools/sign.sh launcher/version.txt dist/version.txt
tools/sign.sh launcher/version-linux.txt dist/version-linux.txt
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
  # The programs first, then each list with its signature: a launcher that reads a list finds what it names.
  files=(dist/GRPLauncher.exe dist/GRPLauncher.zip dist/GRPLauncher-linux)
  if [ -f dist/GRPLauncher-Setup.exe ]; then
    files+=(dist/GRPLauncher-Setup.exe)
  fi
  rsync -a "${files[@]}" "${GRP_HOST:-grp-server}:grp/launcher/"
  rsync -a dist/version.txt dist/version.txt.sig dist/version-linux.txt dist/version-linux.txt.sig "${GRP_HOST:-grp-server}:grp/launcher/"
  echo "published launcher $version (Windows + Linux)"
fi
