# GRPLauncher

Launcher for playing Ghost Recon Phantoms (2016 client, unmodified) on our own servers. Tauri 2: the window is
plain HTML/JS in `ui/`, the work is Rust in `src-tauri/src/`.

- `core.rs`: settings, the account service (`POST /login`, `POST /register`), self-update
  (`/launcher/version.txt`, `/launcher/GRPLauncher.exe`), the game folder check and download against the vault
  (`/files/<key>/manifest.txt`; the key comes back from the sign-in), the resolution (`GraphicSettings.ini`) and
  starting the game with `/ForceDefaultSandbox=true /Sandbox=<server> /Port=<port> /OnlineAccessKey=<key>`.
- `pipe.rs`: the game's launcher protocol on `\\.\pipe\Ghost_Recon_Phantoms`. The game signs in as
  `<name>.<session token>`.
- `main.rs`: the Tauri commands for the window, and the mode without a window used in tests:
  `GRPLauncher.exe --user N --password P [--invite C] [--gamedir <folder>] [--nosync]`.
- `tools/make-vault.sh`: builds the vault (the game files without logs, crash reports or any Phoenix file) and its
  manifest, and uploads them.
- Server side: `GRPBackendWV` (`QuazalWV/Accounts2016.cs`, `GROBackendWV/Server/AccountServer.cs`); on the host,
  nginx on TCP 21080 fronts the account service and serves `/launcher/` and `/files/<key>/`.

The password never leaves the machine: what is sent is PBKDF2-HMAC-SHA1(password, "grp2016:" + lowercase name),
50000 rounds, 32 bytes, hex. The connection is plain HTTP, so that value can still be captured and replayed.
The program is not code-signed: SmartScreen and antivirus warn on first run (`LEEME.txt` tells players what to do).

Build: `./build.sh` (cross-compiles from Linux with cargo-xwin); `./build.sh publish` also uploads it, and every
installed launcher replaces itself on its next start. Raise `VERSION` in `core.rs` for each release.
The window needs the WebView2 runtime (part of Windows 11 and of most Windows 10).

Linux: `cargo build --release` in `src-tauri/` gives a native launcher (needs webkit2gtk-4.1 and `umu-run`). It signs in
and prepares the game folder itself, then runs the game inside Wine through `umu-run`, with the Windows build
(fetched from the server) as the helper that answers the game's pipe (`--session`). `launcher.ini` takes three
more keys there: `prefix` (Wine prefix), `proton` (PROTONPATH) and `phoenixdir` (an existing copy of the game to
take identical files from). Settings and log go to `~/.config/GRPLauncher`, the game to `~/.local/share/GRPLauncher`.

`legacy/` holds the first launcher (WinForms, .NET), kept for reference.
