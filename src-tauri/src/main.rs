// GRP launcher: signs a player in to our account service, keeps itself and the game folder equal to what the
// server publishes, then starts the unmodified Ghost Recon Phantoms client against our servers and answers what
// the game asks its launcher over \\.\pipe\Ghost_Recon_Phantoms.
//
//   GRPLauncher.exe                                   the window
//   GRPLauncher.exe --user N --password P [--invite C] [--gamedir <folder>] [--nosync]   no window (tests)
//
// Settings next to the program, launcher.ini: server, accounthost, accountport, port, accesskey, language.
// What the player chose last (never the password): %APPDATA%\GRPLauncher\settings.txt; log: launcher.log there.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod core;
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
    server: String,
}

#[tauri::command]
fn get_state() -> State {
    let s = core::read_pairs(&core::data_dir().join("settings.txt"));
    State {
        version: core::VERSION,
        name: s.get("name").cloned().unwrap_or_default(),
        gamedir: s.get("gamedir").filter(|v| !v.is_empty()).cloned().unwrap_or_else(|| core::default_game_dir().to_string_lossy().into_owned()),
        resolution: s.get("resolution").cloned().unwrap_or_else(|| "auto".into()),
        windowed: s.get("windowed").map(|v| v == "1").unwrap_or(false),
        server: core::config().server.clone(),
    }
}

#[derive(Deserialize)]
struct PlayArgs {
    name: String,
    password: String,
    invite: Option<String>,
    gamedir: String,
    resolution: String,
    width: u32,
    height: u32,
    windowed: bool,
}

#[derive(Serialize, Clone)]
struct Status {
    text: String,
    fraction: f64, // below 0: no progress bar
}

#[derive(Serialize, Clone)]
struct Done {
    error: Option<String>,
}

// Runs in its own thread; the window hears about it through the events "status", "playing" and "done".
#[tauri::command]
fn play(app: tauri::AppHandle, args: PlayArgs) {
    std::thread::spawn(move || {
        let _ = std::fs::create_dir_all(core::data_dir());
        let _ = std::fs::write(
            core::data_dir().join("settings.txt"),
            format!("name={}\ngamedir={}\nresolution={}\nwindowed={}\n", args.name, args.gamedir, args.resolution, if args.windowed { 1 } else { 0 }),
        );
        let progress_app = app.clone();
        let progress = move |text: &str, fraction: f64| {
            let _ = progress_app.emit("status", Status { text: text.to_string(), fraction });
        };
        let mut error = None;
        if let Some(invite) = args.invite.as_deref().filter(|i| !i.is_empty()) {
            progress("Creando la cuenta...", -1.0);
            let r = core::register(invite, &args.name, &args.password);
            core::log(&format!("register {}: {}", args.name, r));
            if r != "OK" {
                error = Some(core::explain(&r));
            }
        }
        if error.is_none() {
            let playing_app = app.clone();
            let opts = core::PlayOptions { sync: true, width: args.width, height: args.height, windowed: args.windowed };
            error = core::play(&args.name, &args.password, std::path::Path::new(&args.gamedir), &opts, &progress, &move || {
                let _ = playing_app.emit("playing", ());
            })
            .err();
        }
        let _ = app.emit("done", Done { error });
    });
}

#[tauri::command]
fn pick_folder(app: tauri::AppHandle, current: String) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    let mut d = app.dialog().file().set_title("Carpeta donde está o se instalará el juego (unos 4 GB)");
    if std::path::Path::new(&current).is_dir() {
        d = d.set_directory(&current);
    }
    d.blocking_pick_folder().map(|p| p.to_string())
}

fn arg(args: &[String], key: &str) -> Option<String> {
    args.iter().position(|a| a == key).and_then(|i| args.get(i + 1).cloned())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    core::log(&format!("launcher {} started, server {}", core::VERSION, core::config().server));
    if core::self_update(&args) {
        return;
    }
    if let Some(name) = arg(&args, "--user") {
        let pw = arg(&args, "--password").unwrap_or_default();
        if let Some(invite) = arg(&args, "--invite") {
            let r = core::register(&invite, &name, &pw);
            core::log(&format!("register {}: {}", name, r));
            if r != "OK" {
                std::process::exit(2);
            }
        }
        let dir = arg(&args, "--gamedir").map(std::path::PathBuf::from).unwrap_or_else(core::default_game_dir);
        let last = std::sync::Mutex::new(String::new());
        let opts = core::PlayOptions { sync: !args.iter().any(|a| a == "--nosync"), width: 0, height: 0, windowed: true };
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
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![get_state, play, pick_folder])
        .run(tauri::generate_context!())
        .expect("error while running the launcher");
}
