#!/usr/bin/env bash
# Builds the launcher for Windows from Linux and the zip for players.
# Needs: rustup target x86_64-pc-windows-msvc, cargo-xwin (cargo install cargo-xwin), clang, lld, llvm-rc.
#   ./build.sh            build dist/GRPLauncher.exe, dist/GRPLauncher.zip and dist/version.txt
#   ./build.sh publish    the same, then upload the program and version.txt to the server (players' launchers
#                         replace themselves on their next start)
set -euo pipefail
cd "$(dirname "$0")"
( cd src-tauri && cargo xwin build --release --target x86_64-pc-windows-msvc )
mkdir -p dist
cp src-tauri/target/x86_64-pc-windows-msvc/release/GRPLauncher.exe launcher.ini LEEME.txt dist/
version="$(grep -oP 'pub const VERSION: u32 = \K[0-9]+' src-tauri/src/core.rs)"
printf '%s %s\n' "$version" "$(sha256sum dist/GRPLauncher.exe | cut -d' ' -f1)" > dist/version.txt
( cd dist && rm -f GRPLauncher.zip && zip -q GRPLauncher.zip GRPLauncher.exe launcher.ini LEEME.txt )
ls -la dist
if [ "${1:-}" = publish ]; then
  rsync -a dist/GRPLauncher.exe dist/version.txt "${GRP_HOST:-grp-server}:grp/launcher/"
  echo "published launcher $version"
fi
