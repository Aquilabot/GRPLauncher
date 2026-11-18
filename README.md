# GRPLauncher

Launcher for playing Ghost Recon Phantoms (2016 client, unmodified) on our own servers.

- Signs the player in to the account service of the lobby server (`POST /login`, `POST /register` on TCP 21080;
  server side in `GRPBackendWV`: `QuazalWV/Accounts2016.cs`, `GROBackendWV/Server/AccountServer.cs`).
- Starts the game with `/ForceDefaultSandbox=true /Sandbox=<server> /Port=<port> /OnlineAccessKey=<key>` and answers
  the game's launcher protocol on `\\.\pipe\Ghost_Recon_Phantoms`. The game signs in as `<name>.<session token>`.
- Changes no game file and ships none. The password never leaves the machine: what is sent is
  PBKDF2-HMAC-SHA1(password, "grp2016:" + lowercase name), 50000 rounds, 32 bytes, hex. The connection is plain
  HTTP, so that value can still be captured and replayed by someone on the path.

Build (Linux, Mono): `./build.sh` → `dist/GRPLauncher.exe` and `dist/GRPLauncher.zip` (what players get).
`launcher.ini` next to the program sets the server. Without a window, for tests:
`GRPLauncher.exe --user N --password P [--invite C] [--game <GhostReconPhantoms.exe>]`.

Tested under GE-Proton with the game's original `Yeti.ini.enc` and Phoenix's `version.dll` left in the game
folder: sign-up, sign-in, lobby. Not yet tested on a real Windows machine.
