#!/usr/bin/env bash
# make-vault.sh <game folder> <stage folder> [ssh host]
# Builds the vault: the game files players download through the launcher. Copies the game folder to the stage
# folder without what does not belong in it (logs, crash reports, per-player state, and every Phoenix file:
# version.dll, YetiIniEditor.exe, Yeti.ini.enc), writes manifest.txt, and with a host uploads both to ~/grp/vault.
# manifest.txt: first line "vault <number>" (seconds since 1970 at build time), then one line per file:
#   <f|d> <sha256> <size> <path>      f = must match;  d = settings the player may change: only fetched when missing
set -euo pipefail
game="$1"; stage="$2"; host="${3:-}"
mkdir -p "$stage/files"
rsync -a --delete --exclude='/Logs/' --exclude='/BloombergReports/' --exclude='/version.dll' --exclude='/YetiIniEditor.exe' \
  --exclude='/Yeti.ini.enc*' --exclude='*.bak' --exclude='/YetiCapsError.txt' --exclude='/gro.cache.ucc' \
  --exclude='/SplashCounter.ini' "$game/" "$stage/files/"
( cd "$stage/files"
  echo "vault $(date +%s)"
  find . -type f -printf '%P\n' | LC_ALL=C sort | while IFS= read -r p; do
    case "$p" in GraphicSettings.ini|KeyMappingsPC.ini|GeneralOption.ini|Sound.ini|NewbieTips.ini) m=d;; *) m=f;; esac
    printf '%s %s %s %s\n' "$m" "$(sha256sum "$p" | cut -d' ' -f1)" "$(stat -c %s "$p")" "$p"
  done ) > "$stage/manifest.txt"
echo "$(($(wc -l < "$stage/manifest.txt") - 1)) files, $(du -sh "$stage/files" | cut -f1)"
if [ -n "$host" ]; then
  ssh "$host" 'mkdir -p grp/vault/files'
  rsync -a --delete --partial --info=stats1 "$stage/files/" "$host:grp/vault/files/" | tail -2
  rsync -a "$stage/manifest.txt" "$host:grp/vault/manifest.txt"
fi
