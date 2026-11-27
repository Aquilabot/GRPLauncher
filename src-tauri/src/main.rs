// GRP launcher: signs a player in to our account service, keeps itself and the game folder equal to what the
// server publishes, then starts the unmodified Ghost Recon Phantoms client against our servers and answers what
// the game asks its launcher over \\.\pipe\Ghost_Recon_Phantoms.
//
//   GRPLauncher.exe                                   the window
//   GRPLauncher.exe --user N --password P [--create] [--invite C] [--gamedir <folder>] [--nosync]   no window (tests)
//                   [--remember]: keep the session (as the window's "Mantener la sesión iniciada"); no --password:
//                   the kept session; --user N --signout: end it
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
    sidebutton1: String, // the key mouse side button 1 presses; empty: none
    sidebutton2: String,
    dxvk: bool,
    classbuttons: bool,
    server: String,
    lang: String,       // "es", "en", or empty the first time: the window picks by the system's language
    remember: bool,     // "Mantener la sesión iniciada" (on unless the player turned it off)
    keptname: String,   // the account this launcher keeps signed in; empty: none
}

struct SavedClientMods {
    on: bool,
    maxfps: String,
    fov: String,
    focallength: bool,
    adsmult: String,
    scopemult: String,
    sidebuttons: [String; 2],
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
        sidebuttons: ["sidebutton1", "sidebutton2"].map(|k| s.get(k).map(|v| core::parse_side_key(v)).unwrap_or_default()),
    }
}

/// "Vulkan (DXVK)" as settings.txt keeps it: "dxvk", on unless "0" (the launcher writes "0" when the game failed with it).
fn saved_dxvk(s: &std::collections::HashMap<String, String>) -> bool {
    s.get("dxvk").map_or(true, |v| v != "0")
}

/// "Botones de clase" as settings.txt keeps it: "classbuttons", on unless "0".
fn saved_class_buttons(s: &std::collections::HashMap<String, String>) -> bool {
    s.get("classbuttons").map_or(true, |v| v != "0")
}

#[tauri::command]
fn get_state() -> State {
    let s = core::settings();
    let cm = saved_client_mods(&s);
    let [sidebutton1, sidebutton2] = cm.sidebuttons;
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
        sidebutton1,
        sidebutton2,
        dxvk: saved_dxvk(&s),
        classbuttons: saved_class_buttons(&s),
        server: core::config().server.clone(),
        lang: s.get("lang").cloned().unwrap_or_default(),
        remember: s.get("remember").map(|v| v != "0").unwrap_or(true),
        keptname: core::kept_name().unwrap_or_default(),
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
    #[serde(default = "yes")]
    remember: bool,
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
    #[serde(default)]
    sidebutton1: String,
    #[serde(default)]
    sidebutton2: String,
    dxvk: bool,
    #[serde(default = "yes")]
    classbuttons: bool,
}

fn yes() -> bool {
    true
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
            ("sidebutton1", core::parse_side_key(&args.sidebutton1)),
            ("sidebutton2", core::parse_side_key(&args.sidebutton2)),
            ("dxvk", if args.dxvk { "1" } else { "0" }.to_string()),
            ("classbuttons", if args.classbuttons { "1" } else { "0" }.to_string()),
            ("remember", if args.remember { "1" } else { "0" }.to_string()),
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
            core::log(&format!("register {}: {}", args.name, core::loggable(&r)));
            if let Some(key) = core::recovery_key(&r) {
                let _ = app.emit("recovery", key);
            } else if r != "OK" {
                need_invite = r == "ERR invite";
                error = Some(core::explain(&r));
            }
        }
        if error.is_none() {
            let playing_app = app.clone();
            let client = core::client_mods(args.clientmods, &args.maxfps, &args.fov, args.focallength, &args.adsmult, &args.scopemult, [&args.sidebutton1, &args.sidebutton2]);
            let mouse_sens = core::parse_mouse_sens(&args.mousesens);
            let opts = core::PlayOptions { sync: true, width: args.width, height: args.height, windowed: args.windowed, mouse_sens, client, dxvk: args.dxvk, class_buttons: args.classbuttons, run: true };
            error = core::play(&args.name, &args.password, args.remember, std::path::Path::new(&args.gamedir), &opts, &progress, &move || {
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

// "Cambiar contraseña": with the current one. Off the window's thread: it waits for the network.
#[tauri::command(async)]
fn change_password(name: String, password: String, new_password: String) -> Result<(), String> {
    let r = core::change_password(&name, &password, &new_password);
    core::log(&format!("password change {}: {}", name, r));
    if r == "OK" {
        Ok(())
    } else {
        Err(core::explain(&r))
    }
}

// "Nueva clave de recuperación": the key, shown once; it replaces the earlier one.
#[tauri::command(async)]
fn new_recovery_key(name: String, password: String) -> Result<String, String> {
    let r = core::new_recovery_key(&name, &password);
    core::log(&format!("new recovery key {}: {}", name, core::loggable(&r)));
    core::recovery_key(&r).ok_or_else(|| core::explain(&r))
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

// "Cerrar sesión": this launcher no longer keeps the account signed in. Off the window's thread: it tells the server.
#[tauri::command(async)]
fn sign_out() {
    core::sign_out();
}

// Match history from POST /history (cached session token or password secret). Off the window's thread.
#[tauri::command(async)]
fn get_history(name: String, password: String, filter: String, limit: u32) -> Result<serde_json::Value, String> {
    core::fetch_history(&name, &password, &filter, limit)
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

/// Official WebView2 Runtime version strings ("pv") are non-empty and not "0.0.0.0".
fn is_valid_webview2_version(v: &str) -> bool {
    let t = v.trim();
    !t.is_empty() && t != "0.0.0.0"
}

#[cfg(windows)]
fn read_reg_sz(root: windows_sys::Win32::System::Registry::HKEY, subkey: &str, value_name: &str, flags: u32) -> Option<String> {
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, REG_SZ, REG_VALUE_TYPE};
    let sub: Vec<u16> = subkey.encode_utf16().chain(Some(0)).collect();
    let val: Vec<u16> = value_name.encode_utf16().chain(Some(0)).collect();
    let mut hkey: HKEY = std::ptr::null_mut();
    let status = unsafe { RegOpenKeyExW(root, sub.as_ptr(), 0, flags, &mut hkey) };
    if status != ERROR_SUCCESS {
        return None;
    }
    let mut ty: REG_VALUE_TYPE = 0;
    let mut bytes: u32 = 0;
    let query_len = unsafe { RegQueryValueExW(hkey, val.as_ptr(), std::ptr::null(), &mut ty, std::ptr::null_mut(), &mut bytes) };
    if query_len != ERROR_SUCCESS || ty != REG_SZ || bytes < 2 {
        unsafe { RegCloseKey(hkey) };
        return None;
    }
    let mut buf = vec![0u16; (bytes as usize + 1) / 2];
    let query_val = unsafe { RegQueryValueExW(hkey, val.as_ptr(), std::ptr::null(), &mut ty, buf.as_mut_ptr() as *mut u8, &mut bytes) };
    unsafe { RegCloseKey(hkey) };
    if query_val != ERROR_SUCCESS || ty != REG_SZ {
        return None;
    }
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..end]))
}

/// Checks whether Microsoft Edge WebView2 Runtime is installed (per-machine or per-user), matching WebView2Loader.
/// GRP_WEBVIEW2_CHECK=present|missing overrides the check for tests.
#[cfg(windows)]
fn webview2_installed() -> bool {
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY};
    if let Ok(v) = std::env::var("GRP_WEBVIEW2_CHECK") {
        match v.trim().to_ascii_lowercase().as_str() {
            "present" => return true,
            "missing" => return false,
            _ => {}
        }
    }
    if let Ok(folder) = std::env::var("WEBVIEW2_BROWSER_EXECUTABLE_FOLDER") {
        if !folder.trim().is_empty() && std::path::Path::new(folder.trim()).is_dir() {
            return true;
        }
    }
    const CLIENT_KEY: &str = r"SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
    const USER_KEY: &str = r"Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
    let probes = [
        (HKEY_LOCAL_MACHINE, CLIENT_KEY, KEY_READ | KEY_WOW64_32KEY),
        (HKEY_LOCAL_MACHINE, CLIENT_KEY, KEY_READ | KEY_WOW64_64KEY),
        (HKEY_CURRENT_USER, USER_KEY, KEY_READ),
    ];
    probes.into_iter().any(|(root, key, flags)| read_reg_sz(root, key, "pv", flags).is_some_and(|v| is_valid_webview2_version(&v)))
}

#[cfg(windows)]
fn show_missing_webview2_dialog() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, IDYES, MB_ICONWARNING, MB_YESNO};
    const WEBVIEW2_URL: &str = "https://go.microsoft.com/fwlink/p/?LinkId=2124703";
    let text: Vec<u16> = "Falta Microsoft Edge WebView2 Runtime en este equipo, por lo que no se puede abrir la ventana de GRP Launcher.\n\nUsa el instalador GRPLauncher-Setup.exe (lo instala automáticamente) o pulsa Sí para descargar Microsoft Edge WebView2 Runtime ahora.\n\nMicrosoft Edge WebView2 Runtime is missing on this PC, so the GRP Launcher window cannot open.\n\nUse the GRPLauncher-Setup.exe installer (it installs WebView2 automatically) or click Yes to download Microsoft Edge WebView2 Runtime now."
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let title: Vec<u16> = "GRP Launcher".encode_utf16().chain(Some(0)).collect();
    let res = unsafe { MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), MB_YESNO | MB_ICONWARNING) };
    if res == IDYES {
        let _ = std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", WEBVIEW2_URL]).spawn();
    }
}

#[cfg(windows)]
fn show_startup_error_dialog(err: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
    let text: Vec<u16> = format!(
        "No se pudo abrir GRP Launcher:\n{}\n\nCould not open GRP Launcher:\n{}",
        err, err
    )
    .encode_utf16()
    .chain(Some(0))
    .collect();
    let title: Vec<u16> = "GRP Launcher".encode_utf16().chain(Some(0)).collect();
    unsafe { MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), MB_OK | MB_ICONERROR) };
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
        if args.iter().any(|a| a == "--signout") {
            core::sign_out();
            return;
        }
        if let Some(code) = arg(&args, "--reset") {
            let r = core::reset_password(&name, &code, &pw);
            core::log(&format!("password reset {}: {}", name, r));
            std::process::exit(if r == "OK" { 0 } else { 2 });
        }
        let invite = arg(&args, "--invite");
        if args.iter().any(|a| a == "--create") || invite.is_some() {
            let r = core::register(invite.as_deref().unwrap_or(""), &name, &pw);
            core::log(&format!("register {}: {}", name, core::loggable(&r)));
            if r != "OK" && core::recovery_key(&r).is_none() {
                std::process::exit(2);
            }
        }
        let dir = arg(&args, "--gamedir").map(std::path::PathBuf::from).unwrap_or_else(core::default_game_dir);
        let last = std::sync::Mutex::new(String::new());
        let saved = core::settings();
        let cm = saved_client_mods(&saved);
        let client = core::client_mods(cm.on, &cm.maxfps, &cm.fov, cm.focallength, &cm.adsmult, &cm.scopemult, [&cm.sidebuttons[0], &cm.sidebuttons[1]]);
        let mouse_sens = saved.get("mousesens").and_then(|v| core::parse_mouse_sens(v));
        let opts = core::PlayOptions {
            sync: !args.iter().any(|a| a == "--nosync"),
            width: 0,
            height: 0,
            windowed: true,
            mouse_sens,
            client,
            dxvk: saved_dxvk(&saved),
            class_buttons: saved_class_buttons(&saved),
            run: !args.iter().any(|a| a == "--prepare-only"),
        };
        let r = core::play(
            &name,
            &pw,
            args.iter().any(|a| a == "--remember"),
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
    #[cfg(windows)]
    if !webview2_installed() {
        core::log("WebView2 Runtime not found in registry; asked the user to install it");
        show_missing_webview2_dialog();
        return;
    }
    if let Err(e) = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![get_state, set_lang, open_link, get_notes, mark_notes_seen, get_history, sign_out, play, pick_folder, reset_password, change_password, new_recovery_key])
        .run(tauri::generate_context!())
    {
        let msg = e.to_string();
        core::log(&format!("launcher window failed to start: {}", msg));
        #[cfg(windows)]
        show_startup_error_dialog(&msg);
        #[cfg(not(windows))]
        eprintln!("error while running the launcher: {}", msg);
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::is_valid_webview2_version;

    #[test]
    fn recognizes_valid_webview2_versions() {
        assert!(is_valid_webview2_version("131.0.2903.112"));
        assert!(is_valid_webview2_version(" 120.0.2210.91 "));
        assert!(!is_valid_webview2_version(""));
        assert!(!is_valid_webview2_version("   "));
        assert!(!is_valid_webview2_version("0.0.0.0"));
        assert!(!is_valid_webview2_version(" 0.0.0.0 "));
    }
}
