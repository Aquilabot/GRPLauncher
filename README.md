# GRPLauncher

Launcher for playing Ghost Recon Phantoms (2016 client) on our own servers. The game's own files stay as
published; with "Mejoras del cliente" on, our `version.dll` next to them patches it in memory. Tauri 2: the window is
plain HTML/JS in `ui/`, the work is Rust in `src-tauri/src/`.

- `core.rs`: settings, the account service (`POST /login`, `POST /register`), self-update
  (`/launcher/version.txt`, `/launcher/GRPLauncher.exe`), the game folder check and download against the vault
  (`/files/<key>/manifest.txt`; the key comes back from the sign-in), the client improvements (below), the
  resolution (`GraphicSettings.ini`) and
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

## Client improvements

"Mejoras del cliente" in Opciones, on by default: our `version.dll` (source and patch list in the GRP repository,
`client/grp-client-dll/`) in the game folder, with `grp-client.ini` next to it (`[Client]`, `MaxFps=` empty for the
monitor's refresh rate or the number chosen, `UnlockResolutions=1`, CRLF). Switched off, the launcher deletes both and
the game is the original. On Linux the game always runs with `WINEDLLOVERRIDES=version=n,b`, so a `version.dll` in its
folder wins over Wine's, as it does on Windows. Kept in `settings.txt` as `clientmods` (`0` = off) and `maxfps`.

The DLL is not in the vault's manifest (that list is mirrored on every start, whatever the option). It sits in
`/files/<key>/client/` (on the VPS `/srv/grp/vault/client/`): `version.txt` holds `<sha256> <size>` of `version.dll`.
On each start with the option on, the launcher compares that hash with the file in the game folder and, when they
differ, downloads to `version.dll.part` and moves it into place only when size and hash match. A failed check is
logged and the game starts with the DLL it had, or none. Publishing a new DLL is uploading `version.dll`, then
`version.txt`.

## Release notes

The main window shows what changed on the server, in the launcher and in the game, between Opciones and JUGAR ("Novedades").
They come from `<base>/launcher/notes.json` (on the community server `https://grp.limacashout.pro/launcher/notes.json`,
on the VPS `/srv/grp/launcher/notes.json`). `/launcher/` is public, like `version.txt`, so the notes show before
signing in. One JSON list, newest first:

```json
[
  { "id": 2, "date": "2026-11-19", "tag": "servidor", "title": "Partidas más estables",
    "items": ["Ya no se cae la partida cuando un jugador sale.", "Reaparición de 10 s."] },
  { "id": 1, "date": "2026-11-18", "tag": "launcher", "title": "Conexión cifrada", "items": ["..."] }
]
```

- `id`: a whole number that grows with each note; the launcher shows the highest first.
- `tag`: `servidor`, `launcher` or `juego` (anything else: no tag shown). `date`: `YYYY-MM-DD` (anything else: none shown).
- Limits (`src-tauri/src/notes.rs`): file up to 64 KB, the 50 highest ids, title up to 120 characters, up to 12 items
  of up to 400. A longer text is cut; an entry without a numeric `id` or a `title` is skipped; a file that is
  missing, too big or not a list shows no panel, and the window looks as it did before. Everything is shown as text.
- The highest id the player has seen is `notesseen` in `settings.txt`. Newer entries get a mark and the heading a
  count; scrolling or clicking the list, or pressing JUGAR, counts as seen (the marks stay until the window closes).

To add one: `research/tools/deploy/notes-add.sh <tag> "<title>" "<item>" ...` in the GRP repository prints the new file
(next id, today's date); with `--publish` it uploads it.

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
