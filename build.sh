#!/usr/bin/env bash
# Builds GRPLauncher.exe (a .NET Framework 4.x program: runs as is on Windows 10 and 11) and the zip for players.
set -euo pipefail
cd "$(dirname "$0")"
mkdir -p dist
mcs -nologo -target:winexe -platform:x86 -optimize+ -out:dist/GRPLauncher.exe \
  -r:System.Windows.Forms.dll -r:System.Drawing.dll GRPLauncher.cs
cp launcher.ini LEEME.txt dist/
( cd dist && rm -f GRPLauncher.zip && zip -q GRPLauncher.zip GRPLauncher.exe launcher.ini LEEME.txt )
ls -la dist
