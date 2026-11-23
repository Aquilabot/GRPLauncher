// GRP launcher: signs a player in to our account service, keeps itself and the game folder equal to what the
// server publishes, then starts the unmodified Ghost Recon Phantoms client against our servers and answers what
// the game asks its launcher over \\.\pipe\Ghost_Recon_Phantoms.
//
//   GRPLauncher.exe                                   the window
//   GRPLauncher.exe --user N --password P [--create] [--invite C] [--gamedir <folder>] [--nosync]   no window (tests)
//   GRPLauncher.exe --user N --password P --reset <code>   sets the new password P with an admin's code, no window (tests)
//   GRPLauncher.exe --session <name>.<token> --gamedir <folder>     only answer the game and run it (Linux, inside Wine)
//   GRPLauncher.exe --vulkan-check                    prints "ok <device>" or "no <why>": can DXVK run here (Windows)
//
// Settings next to the program, launcher.ini: server, accounthost, accountport, port, accesskey, language.
// What the player chose last (never the password), the window's language ("lang": es or en) and the last release note
// seen: %APPDATA%\GRPLauncher\settings.txt;
// log: launcher.log there.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod core;
mod notes;
mod pipe;

use serde::{Deserialize, Serialize};
use tauri::Emitter;

#[derive(Serialize)]
struct State {
    version: u32,
    name: String,
    gamedir: String,
    resolution: String, // "auto" or "WxH"
    windowed: bool,
    mousesens: String,  // empty: leave KeyMappingsPC.ini untouched
    clientmods: bool,
    maxfps: String,     // empty: the monitor's refresh rate
    fov: String,        // empty: stock (~79.3 deg horizontal at 16:9)
    focallength: bool,  // false: stock ADS curve; true: focal-length scaling
    adsmult: String,    // empty or "1.0": stock multiplier
    scopemult: String,  // empty or "1.0": stock multiplier
    dxvk: bool,
    server: String,
    lang: String,       // "es", "en", or empty the first time: the window picks by the system's language
}

struct SavedClientMods {
    on: bool,
    maxfps: String,
    fov: String,
    focallength: bool,
    adsmult: String,
    scopemult: String,
}

/// "Mejoras del cliente" as settings.txt keeps it.
fn saved_client_mods(s: &std::collections::HashMap<String, String>) -> SavedClientMods {
    SavedClientMods {
        on: s.get("clientmods").map_or(true, |v| v != "0"),
        maxfps: s.get("maxfps").cloned().unwrap_or_default(),
        fov: s.get("fov").cloned().unwrap_or_default(),
        focallength: s.get("focallength").map(|v| v == "1").unwrap_or(false),
        adsmult: s.get("adsmult").cloned().unwrap_or_default(),
        scopemult: s.get("scopemult").cloned().unwrap_or_default(),
    }
}

/// "Vulkan (DXVK)" as settings.txt keeps it: "dxvk", on unless "0" (the launcher writes "0" when the game failed with it).
fn saved_dxvk(s: &std::collections::HashMap<String, String>) -> bool {
    s.get("dxvk").map_or(true, |v| v != "0")
}

#[tauri::command]
fn get_state() -> State {
    let s = core::settings();
    let cm = saved_client_mods(&s);
    State {
        version: core::VERSION,
        name: s.get("name").cloned().unwrap_or_default(),
        gamedir: s.get("gamedir").filter(|v| !v.is_empty()).cloned().unwrap_or_else(|| core::default_game_dir().to_string_lossy().into_owned()),
        resolution: s.get("resolution").cloned().unwrap_or_else(|| "auto".into()),
        windowed: s.get("windowed").map(|v| v == "1").unwrap_or(false),
        mousesens: s.get("mousesens").cloned().unwrap_or_default(),
        clientmods: cm.on,
        maxfps: cm.maxfps,
        fov: cm.fov,
        focallength: cm.focallength,
        adsmult: cm.adsmult,
        scopemult: cm.scopemult,
        dxvk: saved_dxvk(&s),
        server: core::config().server.clone(),
        lang: s.get("lang").cloned().unwrap_or_default(),
    }
}

/// The window's language, kept for next time; the texts this side writes follow it.
#[tauri::command]
fn set_lang(lang: String) {
    let lang = if lang == "en" { "en" } else { "es" };
    core::set_english(lang == "en");
    core::save_settings(&[("lang", lang.to_string())]);
}

/// The links of the window's bottom rail, by name: only these addresses are ever opened, in the player's browser.
#[tauri::command]
fn open_link(which: String) {
    let url = match which.as_str() {
        "status" => "https://grp.limacashout.pro/status/",
        "source" => "https://github.com/Aquilabot/GRPLauncher",
        "support" => "https://ko-fi.com/aquilabot",
        _ => return,
    };
    #[cfg(windows)]
    let r = std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", url]).spawn();
    #[cfg(not(windows))]
    let r = std::process::Command::new("xdg-open").arg(url).spawn();
    if let Err(e) = r {
        core::log(&format!("link {} not opened: {}", which, e));
    }
}

#[derive(Deserialize)]
struct PlayArgs {
    name: String,
    password: String,
    #[serde(default)]
    create: bool,
    invite: Option<String>,
    gamedir: String,
    resolution: String,
    width: u32,
    height: u32,
    windowed: bool,
    #[serde(default)]
    mousesens: String,
    clientmods: bool,
    maxfps: String,
    #[serde(default)]
    fov: String,
    #[serde(default)]
    focallength: bool,
    #[serde(default)]
    adsmult: String,
    #[serde(default)]
    scopemult: String,
    dxvk: bool,
}

#[derive(Serialize, Clone)]
struct Status {
    text: String,
    fraction: f64, // below 0: no progress bar
}

#[derive(Serialize, Clone)]
struct Done {
    error: Option<String>,
    need_invite: bool,
}

// Runs in its own thread; the window hears about it through the events "status", "playing" and "done".
#[tauri::command]
fn play(app: tauri::AppHandle, args: PlayArgs) {
    std::thread::spawn(move || {
        core::save_settings(&[
            ("name", args.name.clone()),
            ("gamedir", args.gamedir.clone()),
            ("resolution", args.resolution.clone()),
            ("windowed", if args.windowed { "1" } else { "0" }.to_string()),
            ("mousesens", args.mousesens.clone()),
            ("clientmods", if args.clientmods { "1" } else { "0" }.to_string()),
            ("maxfps", args.maxfps.clone()),
            ("fov", args.fov.clone()),
            ("focallength", if args.focallength { "1" } else { "0" }.to_string()),
            ("adsmult", args.adsmult.clone()),
            ("scopemult", args.scopemult.clone()),
            ("dxvk", if args.dxvk { "1" } else { "0" }.to_string()),
        ]);
        let progress_app = app.clone();
        let progress = move |text: &str, fraction: f64| {
            let _ = progress_app.emit("status", Status { text: text.to_string(), fraction });
        };
        let mut error = None;
        let mut need_invite = false;
        let invite = args.invite.as_deref().map(str::trim).unwrap_or("");
        if args.create || !invite.is_empty() {
            progress(core::tr("Creando cuenta...", "Creating account..."), -1.0);
            let r = core::register(invite, &args.name, &args.password);
            core::log(&format!("register {}: {}", args.name, r));
            if r != "OK" {
                need_invite = r == "ERR invite";
                error = Some(core::explain(&r));
            }
        }
        if error.is_none() {
            let playing_app = app.clone();
            let client = core::client_mods(args.clientmods, &args.maxfps, &args.fov, args.focallength, &args.adsmult, &args.scopemult);
            let mouse_sens = core::parse_mouse_sens(&args.mousesens);
            let opts = core::PlayOptions { sync: true, width: args.width, height: args.height, windowed: args.windowed, mouse_sens, client, dxvk: args.dxvk };
            error = core::play(&args.name, &args.password, std::path::Path::new(&args.gamedir), &opts, &progress, &move || {
                let _ = playing_app.emit("playing", ());
            })
            .err();
        }
        let _ = app.emit("done", Done { error, need_invite });
    });
}

// The "¿Olvidaste tu contraseña?" form. Off the window's thread: it waits for the network.
#[tauri::command(async)]
fn reset_password(name: String, code: String, password: String) -> Result<(), String> {
    let r = core::reset_password(&name, &code, &password);
    core::log(&format!("password reset {}: {}", name, r));
    if r == "OK" {
        Ok(())
    } else {
        Err(core::explain(&r))
    }
}

#[derive(Serialize)]
struct Notes {
    notes: Vec<notes::Note>,
    seen: u32, // highest id the player has seen; newer ones are marked as new
}

// Off the window's thread: it waits for the network.
#[tauri::command(async)]
fn get_notes() -> Notes {
    Notes { notes: notes::fetch(), seen: core::settings().get("notesseen").and_then(|v| v.parse().ok()).unwrap_or(0) }
}

#[tauri::command]
fn mark_notes_seen(id: u32) {
    let seen: u32 = core::settings().get("notesseen").and_then(|v| v.parse().ok()).unwrap_or(0);
    if id > seen {
        core::save_settings(&[("notesseen", id.to_string())]);
    }
}

#[tauri::command]
fn pick_folder(app: tauri::AppHandle, current: String) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    let mut d = app.dialog().file().set_title(core::tr("Elige la carpeta de instalación del juego (~4 GB)", "Choose the game installation folder (~4 GB)"));
    if std::path::Path::new(&current).is_dir() {
        d = d.set_directory(&current);
    }
    d.blocking_pick_folder().map(|p| p.to_string())
}

fn arg(args: &[String], key: &str) -> Option<String> {
    args.iter().position(|a| a == key).and_then(|i| args.get(i + 1).cloned())
}

/// Whether this program runs from the temporary folder, where WinRAR, 7-Zip and Explorer put a program opened from
/// inside an archive (%TEMP%\Rar$EXa..., %TEMP%\Temp1_<zip>): it and its launcher.ini vanish when the archive closes.
#[cfg(windows)]
fn inside_temp_folder() -> bool {
    let full = |p: std::path::PathBuf| p.canonicalize().ok().map(|p| p.to_string_lossy().to_lowercase());
    match (std::env::current_exe().ok().and_then(full), full(std::env::temp_dir())) {
        (Some(exe), Some(temp)) => exe.starts_with(&format!("{}\\", temp.trim_end_matches('\\'))),
        _ => false,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Before anything else: this is a child of the launcher that only answers one question (see core::vulkan_ready).
    #[cfg(windows)]
    if args.first().map(String::as_str) == Some("--vulkan-check") {
        let r = core::vulkan_probe();
        println!("{}", r.as_ref().map_or_else(|e| format!("no {}", e), |d| format!("ok {}", d)));
        std::process::exit(if r.is_ok() { 0 } else { 1 });
    }
    core::log(&format!("launcher {} started, server {}", core::VERSION, core::config().server));
    #[cfg(windows)]
    if inside_temp_folder() {
        core::log("started from the temporary folder (inside a zip opened with WinRAR or Explorer): asked to extract it first");
        let text: Vec<u16> = "GRP Launcher se abrió directamente desde dentro del archivo .zip.\n\nPrimero descomprime todo el contenido en una carpeta (por ejemplo, en Documentos) y ejecuta GRPLauncher.exe desde ahí.\n\nGRP Launcher was opened directly from inside the .zip file.\n\nExtract all files to a folder first (e.g. Documents) and run GRPLauncher.exe from there."
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let title: Vec<u16> = "GRP Launcher".encode_utf16().chain(Some(0)).collect();
        use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONWARNING, MB_OK};
        unsafe { MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), MB_OK | MB_ICONWARNING) };
        return;
    }
    if core::self_update(&args) {
        return;
    }
    // The last step alone, with the session made by another copy of the launcher (what the Linux build runs inside Wine).
    if let Some(session) = arg(&args, "--session") {
        let dir = arg(&args, "--gamedir").map(std::path::PathBuf::from).unwrap_or_else(core::default_game_dir);
        if let Err(e) = core::run_game(&dir, &session, &|_, _| {}, &|| {}) {
            core::log(&format!("failed: {}", e));
            std::process::exit(1);
        }
        return;
    }
    if let Some(name) = arg(&args, "--user") {
        let pw = arg(&args, "--password").unwrap_or_default();
        if let Some(code) = arg(&args, "--reset") {
            let r = core::reset_password(&name, &code, &pw);
            core::log(&format!("password reset {}: {}", name, r));
            std::process::exit(if r == "OK" { 0 } else { 2 });
        }
        let invite = arg(&args, "--invite");
        if args.iter().any(|a| a == "--create") || invite.is_some() {
            let r = core::register(invite.as_deref().unwrap_or(""), &name, &pw);
            core::log(&format!("register {}: {}", name, r));
            if r != "OK" {
                std::process::exit(2);
            }
        }
        let dir = arg(&args, "--gamedir").map(std::path::PathBuf::from).unwrap_or_else(core::default_game_dir);
        let last = std::sync::Mutex::new(String::new());
        let saved = core::settings();
        let cm = saved_client_mods(&saved);
        let client = core::client_mods(cm.on, &cm.maxfps, &cm.fov, cm.focallength, &cm.adsmult, &cm.scopemult);
        let mouse_sens = saved.get("mousesens").and_then(|v| core::parse_mouse_sens(v));
        let opts = core::PlayOptions { sync: !args.iter().any(|a| a == "--nosync"), width: 0, height: 0, windowed: true, mouse_sens, client, dxvk: saved_dxvk(&saved) };
        let r = core::play(
            &name,
            &pw,
            &dir,
            &opts,
            &|text, _| {
                let mut l = last.lock().unwrap();
                if *l != text {
                    core::log(&format!("status: {}", text));
                    *l = text.to_string();
                }
            },
            &|| {},
        );
        if let Err(e) = r {
            core::log(&format!("failed: {}", e));
            std::process::exit(1);
        }
        return;
    }
    core::set_english(core::settings().get("lang").map(String::as_str) == Some("en"));
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![get_state, set_lang, open_link, get_notes, mark_notes_seen, play, pick_folder, reset_password])
        .run(tauri::generate_context!())
        .expect("error while running the launcher");
}
