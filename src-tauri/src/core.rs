// Everything the launcher does that is not the window: settings, the account service, self-update, the game
// folder check and download, the resolution, and starting the game.
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

/// Of this launcher; the server tells when there is a newer one (/launcher/version.txt) and may refuse older ones.
pub const VERSION: u32 = 4;
pub const GAME_EXE: &str = "GhostReconPhantoms.exe";

pub struct Config {
    pub server: String,
    pub account_host: String, // account service and downloads; empty = the same as server
    pub account_port: u16,    // TCP
    pub account_url: String,  // full address of the account service and downloads; empty = see base_url
    pub port: u16,            // UDP, lobby
    pub access_key: String,
    pub language: String,
    // Linux only: the Wine prefix the game runs in, the Proton umu-run uses, and a folder with an existing copy
    // of the game to take identical files from (on Windows that is Phoenix's install folder).
    pub prefix: String,
    pub proton: String,
    pub phoenix_dir: String,
}

pub fn read_pairs(file: &Path) -> HashMap<String, String> {
    let mut d = HashMap::new();
    if let Ok(text) = std::fs::read_to_string(file) {
        for line in text.lines() {
            if line.trim_start().starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                d.insert(k.trim().to_lowercase(), v.trim().to_string());
            }
        }
    }
    d
}

fn exe_dir() -> PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf())).unwrap_or_else(|| PathBuf::from("."))
}

pub fn config() -> &'static Config {
    static C: OnceLock<Config> = OnceLock::new();
    C.get_or_init(|| {
        let d = read_pairs(&exe_dir().join("launcher.ini"));
        let get = |k: &str, def: &str| d.get(k).filter(|v| !v.is_empty()).cloned().unwrap_or_else(|| def.to_string());
        Config {
            server: get("server", "56.127.0.17"),
            account_host: get("accounthost", ""),
            account_port: get("accountport", "21080").parse().unwrap_or(21080),
            account_url: get("accounturl", ""),
            port: get("port", "21031").parse().unwrap_or(21031),
            access_key: get("accesskey", "8dtRv2oj"),
            language: get("language", "english"),
            prefix: get("prefix", ""),
            proton: get("proton", "GE-Proton"),
            phoenix_dir: get("phoenixdir", ""),
        }
    })
}

fn env_dir(var: &str) -> PathBuf {
    std::env::var_os(var).map(PathBuf::from).unwrap_or_else(|| std::env::temp_dir())
}

#[cfg(not(windows))]
fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(std::env::temp_dir)
}

/// Settings and log. Windows: %APPDATA%\GRPLauncher. Linux: ~/.config/GRPLauncher.
pub fn data_dir() -> PathBuf {
    #[cfg(windows)]
    return env_dir("APPDATA").join("GRPLauncher");
    #[cfg(not(windows))]
    return std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".config")).join("GRPLauncher");
}

/// What the player chose last (never the password): settings.txt in data_dir, "key=value" lines.
pub fn settings() -> HashMap<String, String> {
    read_pairs(&data_dir().join("settings.txt"))
}

/// Sets some keys of settings.txt and keeps the others.
pub fn save_settings(changes: &[(&str, String)]) {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _g = LOCK.lock();
    let mut s: std::collections::BTreeMap<String, String> = settings().into_iter().collect();
    for (k, v) in changes {
        s.insert(k.to_string(), v.clone());
    }
    let _ = std::fs::create_dir_all(data_dir());
    let _ = std::fs::write(data_dir().join("settings.txt"), s.iter().map(|(k, v)| format!("{}={}\n", k, v)).collect::<String>());
}

#[cfg(not(windows))]
fn share_dir() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".local").join("share")).join("GRPLauncher")
}

/// Where the launcher installs the game.
pub fn default_game_dir() -> PathBuf {
    #[cfg(windows)]
    return env_dir("LOCALAPPDATA").join("GRP").join("Ghost Recon Phantoms");
    #[cfg(not(windows))]
    return share_dir().join("Ghost Recon Phantoms");
}

/// An existing copy of the game (Phoenix's install on Windows; launcher.ini "phoenixdir" on Linux): identical files
/// are copied from there instead of downloaded. Never written to.
fn phoenix_game_dir() -> PathBuf {
    if !config().phoenix_dir.is_empty() {
        return PathBuf::from(&config().phoenix_dir);
    }
    env_dir("LOCALAPPDATA").join("Phoenix Network").join("Ghost Recon Phantoms")
}

/// The community server's own address and the encrypted address of its account service. Launchers up to version 3
/// talked to it in plain HTTP on port 21080; a launcher.ini from those days has no "accounturl" and names the
/// server by this address, and gets HTTPS from this version on.
const HOME_SERVER: &str = "56.127.0.17";
const HOME_ACCOUNT_URL: &str = "https://grp.limacashout.pro";

/// Where sign-in, updates and game files come from: "accounturl" of launcher.ini when it is there (any http or
/// https address, no trailing slash); the encrypted address for the community server; otherwise plain HTTP on
/// accounthost (or server) and accountport, which is what a server on your own machine uses.
pub fn base_url() -> String {
    let c = config();
    if !c.account_url.is_empty() {
        return c.account_url.trim_end_matches('/').to_string();
    }
    if c.account_host.is_empty() && (c.server == HOME_SERVER || c.server == "grp.limacashout.pro") {
        return HOME_ACCOUNT_URL.to_string();
    }
    format!("http://{}:{}", if c.account_host.is_empty() { &c.server } else { &c.account_host }, c.account_port)
}

pub fn log(s: &str) {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _g = LOCK.lock();
    let _ = std::fs::create_dir_all(data_dir());
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(data_dir().join("launcher.log")) {
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let _ = writeln!(f, "{} {}", t, s);
    }
}

// ---- account service ----

/// What goes to the server instead of the password: PBKDF2-HMAC-SHA1(password, "grp2016:" + lowercase name),
/// 50000 rounds, 32 bytes, in hex. The server stores it salted and hashed again.
fn secret(name: &str, password: &str) -> String {
    let mut out = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<sha1::Sha1>(password.as_bytes(), format!("grp2016:{}", name.to_lowercase()).as_bytes(), 50000, &mut out);
    hex::encode(out)
}

fn escape(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' { (b as char).to_string() } else { format!("%{:02X}", b) }).collect()
}

/// One line back from the account service: "OK ..." or "ERR <reason>"; a network failure comes back as "NET <text>".
fn post(path: &str, form: &str) -> String {
    let r = ureq::post(&format!("{}{}", base_url(), path))
        .timeout(Duration::from_secs(12))
        .set("Content-Type", "application/x-www-form-urlencoded")
        .send_string(form);
    match r {
        Ok(resp) => resp.into_string().map(|s| s.trim().to_string()).unwrap_or_else(|e| format!("NET {}", e)),
        Err(ureq::Error::Status(_, resp)) => resp.into_string().map(|s| s.trim().to_string()).unwrap_or_else(|e| format!("NET {}", e)),
        Err(e) => format!("NET {}", e),
    }
}

pub fn register(invite: &str, name: &str, password: &str) -> String {
    post("/register", &format!("invite={}&name={}&secret={}", escape(invite), escape(name), secret(name, password)))
}

fn login(name: &str, password: &str) -> String {
    post("/login", &format!("name={}&secret={}&v={}", escape(name), secret(name, password), VERSION))
}

pub fn explain(answer: &str) -> String {
    match answer {
        "ERR invite" => "El código de invitación no es válido o ya se usó.".into(),
        "ERR name" => "El nickname debe tener de 3 a 16 caracteres: letras, números, guion y guion bajo.".into(),
        "ERR taken" => "Ese nickname ya está en uso.".into(),
        "ERR login" => "Nickname o contraseña incorrectos.".into(),
        "ERR slow" => "Demasiados intentos. Espera un minuto.".into(),
        "ERR update" => "Este launcher es antiguo. Ciérralo y vuelve a abrirlo para que se actualice.".into(),
        a if a.starts_with("NET ") => format!("No se pudo conectar con el servidor: {}", &a[4..]),
        a if a.starts_with("ERR ") => format!("El servidor rechazó la solicitud ({}).", a),
        a => format!("Respuesta inesperada del servidor: {}", a.chars().take(80).collect::<String>()),
    }
}

// ---- downloads ----

fn get_text(url: &str, seconds: u64) -> Result<String, String> {
    ureq::get(url).timeout(Duration::from_secs(seconds)).call().map_err(|e| e.to_string())?.into_string().map_err(|e| e.to_string())
}

fn sha256_file(file: &Path) -> std::io::Result<String> {
    let mut f = std::fs::File::open(file)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex::encode(h.finalize()))
}

/// Downloads to <file>.part, continuing a partial one, and moves it into place when the hash is right.
fn download(url: &str, file: &Path, size: u64, sha: &str, progress: &mut dyn FnMut(u64)) -> Result<(), String> {
    let part = PathBuf::from(format!("{}.part", file.display()));
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let mut last = String::new();
    for _attempt in 0..6 {
        let mut have = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
        if have > size {
            let _ = std::fs::remove_file(&part);
            have = 0;
        }
        let attempt = (|| -> Result<(), String> {
            if have < size {
                // no overall time limit (a file can take long); a stalled connection gives up after 30 s without data
                let agent = ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(15)).timeout_read(Duration::from_secs(30)).build();
                let mut req = agent.get(url);
                if have > 0 {
                    req = req.set("Range", &format!("bytes={}-", have));
                }
                let resp = req.call().map_err(|e| e.to_string())?;
                let append = have > 0 && resp.status() == 206;
                let mut f = std::fs::OpenOptions::new().create(true).write(true).append(append).truncate(!append).open(&part).map_err(|e| e.to_string())?;
                let mut reader = resp.into_reader();
                let mut buf = vec![0u8; 1 << 16];
                loop {
                    let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
                    if n == 0 {
                        break;
                    }
                    f.write_all(&buf[..n]).map_err(|e| e.to_string())?;
                    progress(n as u64);
                }
            }
            Ok(())
        })();
        match attempt {
            Err(e) => {
                last = e;
                std::thread::sleep(Duration::from_secs(2));
            }
            Ok(()) => {
                let ok = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0) == size && sha256_file(&part).map(|h| h == sha).unwrap_or(false);
                if ok {
                    let _ = std::fs::remove_file(file);
                    return std::fs::rename(&part, file).map_err(|e| e.to_string());
                }
                let _ = std::fs::remove_file(&part); // wrong content: start over
                last = "el archivo descargado no coincide".into();
            }
        }
    }
    Err(format!("No se pudo descargar {}: {}", file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), last))
}

/// Replaces this program when the server has a newer one, and starts it again. /launcher/version.txt holds
/// "<number> <sha256 of GRPLauncher.exe>". Returns true when the new one was started (the caller exits).
pub fn self_update(args: &[String]) -> bool {
    if cfg!(not(windows)) {
        return false; // the server publishes the Windows program; a Linux build is updated by hand
    }
    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(_) => return false,
    };
    let old = PathBuf::from(format!("{}.old", exe.display()));
    let fresh = PathBuf::from(format!("{}.new", exe.display()));
    let _ = std::fs::remove_file(&old);
    let text = match get_text(&format!("{}/launcher/version.txt", base_url()), 6) {
        Ok(t) => t,
        Err(e) => {
            log(&format!("launcher update check failed: {}", e));
            return false;
        }
    };
    let v: Vec<&str> = text.split_whitespace().collect();
    let n: u32 = v.first().and_then(|x| x.parse().ok()).unwrap_or(0);
    if v.len() < 2 || n <= VERSION {
        return false;
    }
    log(&format!("launcher {} available (this is {})", n, VERSION));
    let r = (|| -> Result<(), String> {
        let resp = ureq::get(&format!("{}/launcher/GRPLauncher.exe", base_url())).timeout(Duration::from_secs(120)).call().map_err(|e| e.to_string())?;
        let mut f = std::fs::File::create(&fresh).map_err(|e| e.to_string())?;
        std::io::copy(&mut resp.into_reader(), &mut f).map_err(|e| e.to_string())?;
        drop(f);
        if sha256_file(&fresh).map_err(|e| e.to_string())? != v[1].to_lowercase() {
            let _ = std::fs::remove_file(&fresh);
            return Err("wrong hash, kept the current one".into());
        }
        std::fs::rename(&exe, &old).map_err(|e| e.to_string())?; // a running program can be renamed, not overwritten
        std::fs::rename(&fresh, &exe).map_err(|e| e.to_string())?;
        std::process::Command::new(&exe).args(args).spawn().map_err(|e| e.to_string())?;
        Ok(())
    })();
    match r {
        Ok(()) => true,
        Err(e) => {
            log(&format!("launcher update failed: {}", e));
            false
        }
    }
}

struct Entry {
    must_match: bool, // false: settings the player may change, only fetched when missing
    sha: String,
    size: u64,
    path: String,
}

fn mtime(file: &Path) -> u128 {
    std::fs::metadata(file).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_nanos()).unwrap_or(0)
}

/// Makes the game folder equal to the vault's file list (manifest.txt: "vault <n>", then "<f|d> <sha256> <size> <path>").
fn sync_game(vault_key: &str, game_dir: &Path, progress: &dyn Fn(&str, f64)) -> Result<(), String> {
    let base = format!("{}/files/{}/", base_url(), vault_key);
    let text = get_text(&format!("{}manifest.txt", base), 30).map_err(|e| format!("No se pudo leer la lista de archivos del servidor: {}", e))?;
    let mut entries = Vec::new();
    let mut vault = String::new();
    for line in text.lines() {
        let a: Vec<&str> = line.trim_end().splitn(4, ' ').collect();
        if a.len() == 2 && a[0] == "vault" {
            vault = a[1].to_string();
        }
        if a.len() == 4 && (a[0] == "f" || a[0] == "d") {
            // paths come from the server: nothing absolute, nothing that climbs out of the game folder
            let p = a[3];
            if p.contains("..") || p.starts_with('/') || p.starts_with('\\') || p.contains(':') {
                continue;
            }
            if let Ok(size) = a[2].parse() {
                entries.push(Entry { must_match: a[0] == "f", sha: a[1].to_string(), size, path: p.to_string() });
            }
        }
    }
    if entries.is_empty() {
        return Err("El servidor no publicó la lista de archivos del juego.".into());
    }
    log(&format!("vault {}: {} files", vault, entries.len()));
    std::fs::create_dir_all(game_dir).map_err(|e| format!("No se pudo crear la carpeta del juego:\n{}\n{}", game_dir.display(), e))?;

    // What was hashed before: path -> "size|modified|sha". A file whose size and time did not change is not hashed again.
    let cache_file = game_dir.join(".grp-files");
    let mut cache: HashMap<String, String> = HashMap::new();
    if let Ok(t) = std::fs::read_to_string(&cache_file) {
        for l in t.lines() {
            if let Some((k, v)) = l.split_once('\t') {
                cache.insert(k.to_string(), v.to_string());
            }
        }
    }
    let local = |e: &Entry| game_dir.join(e.path.replace('/', std::path::MAIN_SEPARATOR_STR));
    let mut need = Vec::new();
    for (i, e) in entries.iter().enumerate() {
        if i % 20 == 0 {
            progress("Comprobando archivos del juego...", i as f64 / entries.len() as f64);
        }
        let file = local(e);
        if !e.must_match && file.exists() {
            continue;
        }
        let size = std::fs::metadata(&file).map(|m| m.len()).ok();
        let mut same = false;
        if size == Some(e.size) {
            let key = format!("{}|{}|{}", e.size, mtime(&file), e.sha);
            if cache.get(&e.path) == Some(&key) {
                same = true;
            } else if sha256_file(&file).map(|h| h == e.sha).unwrap_or(false) {
                cache.insert(e.path.clone(), key);
                same = true;
            }
        }
        if !same {
            need.push(e);
        }
    }
    let total: u64 = need.iter().map(|e| e.size).sum();
    log(&format!("{} files to get, {} MB", need.len(), total >> 20));
    let phoenix = phoenix_game_dir();
    let use_phoenix = phoenix.is_dir() && std::fs::canonicalize(&phoenix).ok() != std::fs::canonicalize(game_dir).ok();
    let (mut done, mut copied, mut fetched) = (0u64, 0u32, 0u32);
    let save_cache = |cache: &HashMap<String, String>| {
        let text: String = cache.iter().map(|(k, v)| format!("{}\t{}\n", k, v)).collect();
        let _ = std::fs::write(&cache_file, text);
    };
    for e in &need {
        let file = local(e);
        let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let fraction = |done: u64| if total == 0 { 1.0 } else { done as f64 / total as f64 };
        let other = phoenix.join(e.path.replace('/', std::path::MAIN_SEPARATOR_STR));
        let mut got = false;
        if use_phoenix && std::fs::metadata(&other).map(|m| m.len()).ok() == Some(e.size) {
            progress(&format!("Copiando {} de la instalación de Phoenix...", name), fraction(done));
            if sha256_file(&other).map(|h| h == e.sha).unwrap_or(false) {
                if let Some(dir) = file.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                if std::fs::copy(&other, &file).is_ok() {
                    done += e.size;
                    copied += 1;
                    got = true;
                }
            }
        }
        if !got {
            let url = format!("{}files/{}", base, e.path.split('/').map(escape).collect::<Vec<_>>().join("/"));
            let start = done;
            let mut step = |n: u64| {
                done += n;
                progress(&format!("Descargando {}  ({} de {} MB)", name, done >> 20, total >> 20), fraction(done));
            };
            if let Err(err) = download(&url, &file, e.size, &e.sha, &mut step) {
                save_cache(&cache);
                log(&format!("sync failed: {}", err));
                return Err(err);
            }
            done = start + e.size;
            fetched += 1;
        }
        cache.insert(e.path.clone(), format!("{}|{}|{}", e.size, mtime(&file), e.sha));
    }
    save_cache(&cache);
    log(&format!("game folder ready: {} copied from Phoenix, {} downloaded", copied, fetched));
    Ok(())
}

// ---- the game ----

pub struct PlayOptions {
    pub sync: bool,
    pub width: u32,  // 0: leave the game's resolution as it is
    pub height: u32,
    pub windowed: bool,
}

/// The game reads its resolution from GraphicSettings.ini in its folder (DisplayWidth, DisplayHeight, Windowed).
/// Any size works there, also above the 1920x1200 its own options screen offers.
fn set_resolution(game_dir: &Path, o: &PlayOptions) {
    if o.width == 0 || o.height == 0 {
        return;
    }
    let file = game_dir.join("GraphicSettings.ini");
    let text = match std::fs::read_to_string(&file) {
        Ok(t) => t,
        Err(_) => return,
    };
    let mut seen = [false; 3];
    let mut lines: Vec<String> = text
        .lines()
        .map(|l| {
            let l = l.trim_end_matches('\r');
            if l.starts_with("DisplayWidth=") {
                seen[0] = true;
                format!("DisplayWidth={}", o.width)
            } else if l.starts_with("DisplayHeight=") {
                seen[1] = true;
                format!("DisplayHeight={}", o.height)
            } else if l.starts_with("Windowed=") {
                seen[2] = true;
                format!("Windowed={}", if o.windowed { 1 } else { 0 })
            } else {
                l.to_string()
            }
        })
        .collect();
    if !seen[0] {
        lines.push(format!("DisplayWidth={}", o.width));
    }
    if !seen[1] {
        lines.push(format!("DisplayHeight={}", o.height));
    }
    if !seen[2] {
        lines.push(format!("Windowed={}", if o.windowed { 1 } else { 0 }));
    }
    let _ = std::fs::write(&file, lines.join("\r\n") + "\r\n");
    log(&format!("resolution {}x{}{}", o.width, o.height, if o.windowed { " windowed" } else { "" }));
}

/// Signs in, brings the game folder up to date and runs the game until it exits.
pub fn play(name: &str, password: &str, game_dir: &Path, o: &PlayOptions, progress: &dyn Fn(&str, f64), playing: &dyn Fn()) -> Result<(), String> {
    progress("Iniciando sesión...", -1.0);
    let answer = login(name, password);
    let a: Vec<&str> = answer.split(' ').collect();
    if a.len() < 3 || a[0] != "OK" {
        log(&format!("login refused: {}", if answer.starts_with("OK") { "bad answer" } else { answer.as_str() }));
        return Err(explain(&answer));
    }
    log(&format!("signed in as {}", a[1]));
    if o.sync && a.len() >= 4 {
        sync_game(a[3], game_dir, progress)?;
    }
    if !game_dir.join(GAME_EXE).is_file() {
        return Err(format!("No se encontró el juego en:\n{}", game_dir.join(GAME_EXE).display()));
    }
    set_resolution(game_dir, o);
    run_game(game_dir, &format!("{}.{}", a[1], a[2]), progress, playing)
}

/// Runs the game with the session already made ("<name>.<token>") and waits for it to exit.
#[cfg(windows)]
pub fn run_game(game_dir: &Path, session: &str, progress: &dyn Fn(&str, f64), playing: &dyn Fn()) -> Result<(), String> {
    crate::pipe::start(session.to_string());
    std::thread::sleep(Duration::from_millis(500));
    progress("Abriendo el juego...", -1.0);
    let c = config();
    let mut child = std::process::Command::new(game_dir.join(GAME_EXE))
        .current_dir(game_dir)
        .arg("/ForceDefaultSandbox=true")
        .arg(format!("/Sandbox={}", c.server))
        .arg(format!("/Port={}", c.port))
        .arg(format!("/OnlineAccessKey={}", c.access_key))
        .arg(format!("/language={}", c.language))
        .spawn()
        .map_err(|e| {
            log(&format!("game start failed: {}", e));
            format!("No se pudo abrir el juego: {}", e)
        })?;
    log(&format!("game started, pid {}", child.id()));
    progress("Jugando. Esta ventana vuelve cuando cierres el juego.", -1.0);
    playing();
    let code = child.wait().map(|s| s.code().unwrap_or(-1)).unwrap_or(-1);
    log(&format!("game exited, code {}", code));
    Ok(())
}

/// Linux: the game is a Windows program and talks to its launcher over a Windows pipe, so the last step runs
/// inside Wine: umu-run starts the Windows build of this launcher with the session ("--session"), and that one
/// answers the pipe and starts the game. The Windows build is fetched from the server next to this program.
#[cfg(not(windows))]
pub fn run_game(game_dir: &Path, session: &str, progress: &dyn Fn(&str, f64), playing: &dyn Fn()) -> Result<(), String> {
    progress("Preparando Proton...", -1.0);
    let helper = share_dir().join("GRPLauncher.exe");
    let _ = std::fs::create_dir_all(share_dir());
    let published = get_text(&format!("{}/launcher/version.txt", base_url()), 10).unwrap_or_default();
    let want = published.split_whitespace().nth(1).unwrap_or("").to_lowercase();
    if !helper.is_file() || (!want.is_empty() && sha256_file(&helper).map(|h| h != want).unwrap_or(true)) {
        let resp = ureq::get(&format!("{}/launcher/GRPLauncher.exe", base_url())).timeout(Duration::from_secs(120)).call().map_err(|e| format!("No se pudo descargar el ayudante de Windows: {}", e))?;
        let mut f = std::fs::File::create(&helper).map_err(|e| e.to_string())?;
        std::io::copy(&mut resp.into_reader(), &mut f).map_err(|e| e.to_string())?;
    }
    let ini = exe_dir().join("launcher.ini");
    if ini.is_file() {
        let _ = std::fs::copy(&ini, share_dir().join("launcher.ini")); // the helper reads the same server settings
    }
    let c = config();
    let prefix = if c.prefix.is_empty() { share_dir().join("prefix") } else { PathBuf::from(&c.prefix) };
    let win = |p: &Path| format!("Z:{}", p.to_string_lossy().replace('/', "\\"));
    progress("Abriendo el juego...", -1.0);
    let mut child = std::process::Command::new("umu-run")
        .env("WINEPREFIX", &prefix)
        .env("GAMEID", "umu-default")
        .env("PROTONPATH", &c.proton)
        .arg(&helper)
        .arg("--session")
        .arg(session)
        .arg("--gamedir")
        .arg(win(game_dir))
        .spawn()
        .map_err(|e| format!("No se pudo ejecutar umu-run (¿está instalado?): {}", e))?;
    log(&format!("umu-run started, pid {}, prefix {}", child.id(), prefix.display()));
    progress("Jugando. Esta ventana vuelve cuando cierres el juego.", -1.0);
    playing();
    let code = child.wait().map(|s| s.code().unwrap_or(-1)).unwrap_or(-1);
    log(&format!("umu-run exited, code {}", code));
    Ok(())
}
