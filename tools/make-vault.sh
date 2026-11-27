#!/usr/bin/env bash
# make-vault.sh <game folder> <stage folder> [ssh host]
# Builds the vault: the game files players download through the launcher. Copies the game folder to the stage
# folder without what does not belong in it (logs, crash reports, per-player state, third-party launcher files:
# version.dll, YetiIniEditor.exe, Yeti.ini.enc, and our client improvements: version.dll, grp-client.ini and its
# log, and DXVK: d3d9.dll, its licence and log, which the launcher adds from vault/client/ only when the player has
# them on), writes manifest.txt, and with
# a host uploads both to ~/grp/vault.
# manifest.txt: first line "vault <number>" (seconds since 1970 at build time), then one line per file:
#   <f|d> <sha256> <size> <path>      f = must match;  d = settings the player may change: only fetched when missing
# For any 'f' file larger than GRP_BLOCK_THRESHOLD (default 64 MiB = 67108864 bytes), a second line immediately follows:
#   b <block_size> <sha_block_0> <sha_block_1> ...   (default 4 MiB = 4194304 bytes; ignored by launchers before v26)
# Set GRP_SKIP_SIGN=1 only in local tests without a signing key.
set -euo pipefail
game="$1"; stage="$2"; host="${3:-}"
block_threshold="${GRP_BLOCK_THRESHOLD:-67108864}"
block_size="${GRP_BLOCK_SIZE:-4194304}"
mkdir -p "$stage/files"
rsync -a --delete --exclude='/Logs/' --exclude='/BloombergReports/' --exclude='/version.dll*' --exclude='/YetiIniEditor.exe' \
  --exclude='/Yeti.ini.enc*' --exclude='*.bak' --exclude='/YetiCapsError.txt' --exclude='/gro.cache.ucc' \
  --exclude='/SplashCounter.ini' --exclude='/grp-client.*' --exclude='/d3d9.dll*' --exclude='/DXVK-LICENSE.txt' \
  --exclude='/*_d3d9.log' "$game/" "$stage/files/"
python3 - "$stage/files" "$stage/manifest.txt" "$block_threshold" "$block_size" <<'PY'
import hashlib, os, sys, time

root, out_path, threshold, block_size = sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4])
defaults = {"GraphicSettings.ini", "KeyMappingsPC.ini", "GeneralOption.ini", "Sound.ini", "NewbieTips.ini"}

paths = []
for dirpath, _, filenames in os.walk(root):
    for fn in filenames:
        full = os.path.join(dirpath, fn)
        if os.path.islink(full):
            continue
        rel = os.path.relpath(full, root).replace(os.sep, "/")
        paths.append(rel)
paths.sort()

file_count = 0
block_files = 0
with open(out_path, "w", encoding="utf-8", newline="\n") as out:
    out.write(f"vault {int(time.time())}\n")
    for rel in paths:
        full = os.path.join(root, rel)
        size = os.path.getsize(full)
        mode = "d" if rel in defaults else "f"
        want_blocks = mode == "f" and size > threshold and block_size > 0
        whole = hashlib.sha256()
        block_hashes = []
        with open(full, "rb") as f:
            if want_blocks:
                while True:
                    chunk = f.read(block_size)
                    if not chunk:
                        break
                    whole.update(chunk)
                    block_hashes.append(hashlib.sha256(chunk).hexdigest())
            else:
                while True:
                    chunk = f.read(1 << 20)
                    if not chunk:
                        break
                    whole.update(chunk)
        out.write(f"{mode} {whole.hexdigest()} {size} {rel}\n")
        file_count += 1
        if want_blocks:
            out.write(f"b {block_size} {' '.join(block_hashes)}\n")
            block_files += 1
print(f"{file_count} files ({block_files} with {block_size // (1024 * 1024) or block_size} block hashes)")
PY
if [ "${GRP_SKIP_SIGN:-0}" != 1 ]; then
  "$(dirname "$0")/sign.sh" manifest.txt "$stage/manifest.txt"   # the launcher (25 on) asks for manifest.txt.sig
fi
echo "$(du -sh "$stage/files" | cut -f1)"
if [ -n "$host" ]; then
  ssh "$host" 'mkdir -p grp/vault/files'
  rsync -a --delete --partial --info=stats1 "$stage/files/" "$host:grp/vault/files/" | tail -2
  rsync -a "$stage/manifest.txt" "$stage/manifest.txt.sig" "$host:grp/vault/"
fi
