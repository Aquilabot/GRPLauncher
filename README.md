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
50000 rounds, 32 bytes, hex. Since version 4 the connection to the community server is HTTPS
(`https://grp.limacashout.pro`, certificates checked against the system's store); `accounturl` in `launcher.ini`
points it somewhere else, and a server on your own machine is still reached in plain HTTP.
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

## What it does

GRPLauncher is the program players use to play *Ghost Recon Phantoms* (a game whose official servers closed in
2016) on a community-run server: it creates or signs in to the player's account on that server, downloads the game
files from it and keeps them identical for everybody, keeps itself up to date, sets the screen resolution and
starts the game. It does not modify the game's executable and it is not a cheat, crack or trainer. The game files
are not part of this repository or of the program; they come from the server the player connects to.

Downloads: the [releases](https://git.limacashout.pro/aquilabot/GRPLauncher/releases) page.

The repository lives at <https://git.limacashout.pro/aquilabot/GRPLauncher> and is mirrored at
<https://github.com/Aquilabot/GRPLauncher>, where the release builds run.

## License

[Mozilla Public License 2.0](LICENSE). The files you change stay under the MPL and their source must be available
to whoever receives your build; you may combine them with code under other terms.

## Code signing policy

Planned (application pending): free code signing provided by [SignPath.io](https://about.signpath.io/), certificate
by [SignPath Foundation](https://signpath.org/).

- Only the launcher built from this repository is signed. The signed files are produced by the automated build
  in `.github/workflows/release.yml`, which runs for a version tag or when a maintainer starts it, never for a
  pull request; each release signing is approved by hand.
- Team roles: **author, reviewer and approver**: [aquilabot](https://github.com/Aquilabot)
  ([on the project's own server](https://git.limacashout.pro/aquilabot)), the project's only maintainer. Everybody else contributes through pull requests, which the maintainer reviews
  before merging ([CONTRIBUTING.md](CONTRIBUTING.md)).
- Privacy: see the [privacy policy](PRIVACY.md). This program will not transfer any information to other
  networked systems unless specifically requested by the user or the person installing or operating it.
