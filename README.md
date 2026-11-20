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
The game needs the 32-bit DirectX 9 runtime of June 2010 (`d3dx9_43.dll`, `xinput1_3.dll`), which Windows 10 and 11
lack. Before starting the game, when either DLL is missing from `SysWOW64` (`System32` on 32-bit Windows), the launcher
runs `DXRedist\DXSETUP.exe /silent` from the game folder elevated (one UAC prompt) and waits; a declined prompt or a
failed setup stops there with what to do. `GRP_DX_CHECK=<folder>` looks for the DLLs in that folder instead (an empty
one forces the install, for tests). Wine prefixes carry their own copies, so under Wine nothing runs. DXSETUP probes
for .NET to decide about Managed DirectX, which makes Windows offer .NET Framework 3.5; the game does not need it.

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

## Vulkan (DXVK)

"Vulkan (DXVK)" in Opciones, on by default on Windows: DXVK's 32-bit `d3d9.dll` (release 3.1.1, the version Proton
runs the game with on Linux) in the game folder next to our `version.dll`, so the game draws through Vulkan, with
DXVK's licence beside it (`DXVK-LICENSE.txt`, from `third-party/`). Kept in `settings.txt` as `dxvk` (`0` = off).
Switched off, or on a machine where DXVK cannot run, the launcher deletes both and the game uses Direct3D 9.

- **Vulkan check, before each start:** the 32-bit Vulkan loader (`SysWOW64\vulkan-1.dll`) must exist, and
  `GRPLauncher.exe --vulkan-check`, run as a child process so a broken driver or layer cannot take the launcher down
  (killed after 20 s), must find a device DXVK would take: Vulkan 1.3, not a software renderer, with the extensions
  DXVK 3.1.1 requires (`VK_KHR_swapchain`, `VK_EXT_robustness2`, `VK_KHR_maintenance5`, `VK_KHR_maintenance6`,
  `VK_KHR_load_store_op_none`, `VK_EXT_depth_clip_enable`). It prints `ok <device>` or `no <why>`; the launcher logs
  either. `GRP_VULKAN_CHECK=pass|fail` skips it (tests).
- **After each game with DXVK:** DXVK writes `GhostReconPhantoms_d3d9.log` in the game folder (cleared before each
  start). Without its "Presenter: Actual swapchain properties" line, DXVK never showed a frame: the launcher deletes
  the DLL, saves `dxvk=0`, logs the first `err:` line and tells the player to press JUGAR again (DirectX 9 then).
- The DLL is published like ours: `client/d3d9.dll` and `client/d3d9.txt` (`<sha256> <size>`) in the vault, fetched
  whole and checked; `make-vault.sh` keeps the DXVK files out of the manifest.
- Linux: nothing is placed. Proton already puts its DXVK in the prefix; the option shows as on and cannot be
  changed, and after each game the log says which `d3d9.dll` the prefix had (`DXVK (Vulkan)` or `WineD3D`, which
  means `PROTON_USE_WINED3D` or a broken prefix).

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
