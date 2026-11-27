#!/usr/bin/env bash
# sign.sh <name> <file>: writes <file>.sig, the signature the launcher asks of a list before it believes it.
#   <name> is the list's place on the server, the way the launcher asks for it: launcher/version.txt,
#   launcher/version-linux.txt, manifest.txt, client/version.txt, client/d3d9.txt, client/ui/<list>.txt.
#   It is signed with the bytes ("<name>\n" and then the file), so one list cannot be served as another.
# The key: GRP_SIGN_KEY, or ~/.config/grp/sign-key.pem (ed25519, made with
# "openssl genpkey -algorithm ed25519"). It stays on the machine that publishes, never on the game server: whoever
# holds it decides what every launcher installs. Its public half is HOME_SIGN_KEY in src-tauri/src/core.rs:
#   sign.sh --public    prints it
# Upload <file>.sig right after <file>.
set -euo pipefail
key="${GRP_SIGN_KEY:-$HOME/.config/grp/sign-key.pem}"
[ -f "$key" ] || { echo "sign.sh: no key at $key" >&2; exit 1; }
if [ "${1:-}" = --public ]; then
  openssl pkey -in "$key" -pubout -outform DER | tail -c 32 | xxd -p -c 64
  exit 0
fi
[ $# -eq 2 ] && [ -f "$2" ] || { echo "usage: sign.sh <name> <file> | sign.sh --public" >&2; exit 2; }
msg="$(mktemp)"; trap 'rm -f "$msg" "$msg.sig"' EXIT
{ printf '%s\n' "$1"; cat "$2"; } > "$msg"
openssl pkeyutl -sign -inkey "$key" -rawin -in "$msg" -out "$msg.sig"
xxd -p -c 128 "$msg.sig" > "$2.sig"
