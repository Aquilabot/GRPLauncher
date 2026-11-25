# Privacy policy

GRPLauncher talks only to the game server written in `launcher.ini` next to the program (the key `server`), and
only when you use it:

- **Signing in or creating an account** (`POST /login`, `POST /register`): it sends the nickname you typed, a value
  derived from your password (PBKDF2-HMAC-SHA1 of the password with the nickname, 50 000 rounds; the password
  itself is never sent or stored), the launcher's version number and, only when an invitation code is entered, that
  code. The server answers with a session token valid for 12 hours, which the launcher hands to the game.
- **Keeping itself and the game up to date**: it downloads `version.txt` and, when there is a newer launcher, the
  new program; it downloads the list of game files and the files that are missing or different, and, with
  "Mejoras del cliente" on (since version 7), the hash of the improvements file and the file when it changed.
  Those requests carry nothing but the file names and the access key the server returned at sign-in.
- **Showing the server's news** (since version 5): when the window opens and when the game closes it downloads
  `notes.json`, a public list of changes. The request carries nothing about you.

As with any connection, the server sees your IP address; it keeps it in its access log.

What the launcher keeps on your computer: the nickname (never the password), the folder and the resolution you
chose (client improvements and frame cap included), the number of the last news entry you saw, a log of its own actions, and the game files. On Windows they are next to the program; on Linux in
`~/.config/GRPLauncher` and `~/.local/share/GRPLauncher`.

It has no telemetry, analytics, advertising or crash reporting, and it contacts no other system. This program
will not transfer any information to other networked systems unless specifically requested by the user or the
person installing or operating it.

The connection to the community server is encrypted (HTTPS) since version 4 of the launcher. The game itself
talks to the game server over its own protocol (UDP), as the original game did.

Changes it makes to your system:
- **Portable zip (`GRPLauncher.zip`):** none outside its own folder, `%APPDATA%\GRPLauncher`, and the game folder you choose. To remove it, delete those folders; nothing is written to the registry.
- **Windows installer (`GRPLauncher-Setup.exe`):** installs per-user into `%LOCALAPPDATA%\GRPLauncher`, creates Start Menu and optional Desktop shortcuts, and writes a standard per-user Add/Remove Programs entry under `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\GRPLauncher`. If Microsoft Edge WebView2 Runtime is missing, the installer runs Microsoft's official Evergreen WebView2 Bootstrapper to install it. Uninstalling removes the launcher, its shortcuts, and that registry entry while keeping your game folder and settings intact.
