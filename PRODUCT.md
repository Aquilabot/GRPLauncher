# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Players of *Tom Clancy's Ghost Recon Phantoms* (shut down by Ubisoft in 2016) who want to play it again on a
community-run server. Two groups, confirmed by the owner on 23-nov-2026:

- Spanish-speaking players in Latin America, the server's home region (São Paulo, 60 Hz). Spanish is the default
  language.
- The game's old international community, reached in English.

They open the launcher on a Windows PC (or on Linux through Proton) every time they play: sign in, press play, and the
window minimizes while the game runs. A first-time player also creates the account here and waits for a download of
about 4 GB.

## Product Purpose

The one program a player needs to get from nothing to a match: create or sign in to an account, download and update
the game from the community's store, apply the player's settings, and start the game against the community server.
It also carries the server's release notes. Success is a player in a match without reading a guide.

The project is about to go public to revive the community, and will accept voluntary donations (Ko-fi) toward server
costs.

## Positioning

A free, open-source (MPL-2.0), portable launcher for a community server that runs its own backend: no installer, no
registry entries, no telemetry or ads. It adds things the 2016 game never had: resolutions above 1920 × 1200, more
than 60 FPS, a configurable field of view, exact mouse sensitivity with a converter from other shooters, and Vulkan
through DXVK.

## Operating Context

- A Tauri 2 window (WebView2 on Windows, WebKitGTK on Linux), fixed size, not resizable. The owner approved a larger
  window than the current 460 × 680.
- Three plain files in `ui/` (`index.html`, `style.css`, `main.js`), no framework, no bundler. The Rust side does the
  work through commands (`get_state`, `get_notes`, `mark_notes_seen`, `play`, `pick_folder`, `reset_password`) and
  events (`status`, `playing`, `done`).
- States the window must show: idle, connecting, downloading with progress, an error from the server, playing
  (minimized), a new-account flow, an invitation code asked for by the server, and password reset with a code from an
  administrator.
- Release notes come from the server as text, newest first, tagged `servidor`, `launcher` or `juego`; notes the
  player has not seen are marked.

## Capabilities and Constraints

- Account: nickname (16 characters at most), password (6 at least when creating), optional "create account",
  invitation code only when the server asks for one, password reset by code.
- Settings: game folder, resolution, windowed mode (hidden on Linux), exact mouse sensitivity with a calculator
  (cm/360° or another game's value), client improvements (FPS limit, FOV, focal-length ADS scaling, ADS and scope
  multipliers), Vulkan (DXVK; always on and fixed on Linux).
- Language: Spanish and English, Spanish by default (owner, 23-nov-2026).
- Donations: a discreet, permanent link to Ko-fi (`ko-fi.com/aquilabot`) next to the other links; it never stands
  between the player and the play button (owner, 23-nov-2026).
- No Ubisoft artwork, screenshots or logotypes: the visual identity is the project's own (owner, 23-nov-2026).
- No external fonts or scripts fetched at run time: the launcher works offline until it signs in, and makes no
  third-party requests (`PRIVACY.md`).
- Undecided: whether the launcher links to a Discord server from inside the window (the invitation link is not in
  the repository).

## Brand Commitments

- Names: "GRPLauncher" for the program; the game is "Ghost Recon Phantoms" (GRP).
- Not affiliated with Ubisoft; the project says so.
- Voice: plain and direct, the way one player explains something to another. No marketing language.

## Evidence on Hand

- `README.md` (English) and `LEEME.txt` (Spanish): what the project is and how to install it.
- Server status page: `https://grp.limacashout.pro/status/`.
- The current icon: `src-tauri/icons/icon.png`.
- No photography, illustration or brand kit exists. No testimonials or player counts to quote.

## Product Principles

1. The play button is always one glance away.
2. The launcher tells the player what is happening and what to do next, in their language.
3. Settings the original game never had are a reason to choose this launcher: they deserve room, not a hidden drawer.
4. Nothing that looks official: this is a community's work and reads as such.
5. Asking for support is honest and never in the way.
