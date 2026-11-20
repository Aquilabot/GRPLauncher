// Everything the launcher does that is not the window: settings, the account service, self-update, the game
// folder check and download, the resolution, and starting the game.
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

/// Of this launcher; the server tells when there is a newer one (/launcher/version.txt) and may refuse older ones.
pub const VERSION: u32 = 9;
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

/// A new password with the single-use code an admin gave the player: "OK", "ERR code" (wrong, used or expired)...
pub fn reset_password(name: &str, code: &str, password: &str) -> String {
    post("/reset", &format!("name={}&code={}&secret={}", escape(name), escape(code), secret(name, password)))
}

/// "ERR banned <until, unix seconds, 0 for good> <reason>" in words: how long is left, and why.
fn banned(rest: &str) -> String {
    let (until, reason) = rest.split_once(' ').unwrap_or((rest, ""));
    let until: u64 = until.parse().unwrap_or(0);
    let when = if until == 0 {
        "de forma permanente.".to_string()
    } else {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        format!("y volverá a estar activa en {}.", left(until.saturating_sub(now)))
    };
    let reason = reason.trim();
    format!("Tu cuenta está suspendida {}{}", when, if reason.is_empty() { String::new() } else { format!("\nMotivo: {}", reason) })
}

/// A time span in words, the two largest units: "6 días y 23 horas", "5 horas y 1 minuto", "12 minutos".
fn left(seconds: u64) -> String {
    let unit = |n: u64, one: &str, many: &str| format!("{} {}", n, if n == 1 { one } else { many });
    let minutes = seconds.div_ceil(60).max(1);
    let (d, h, m) = (minutes / 1440, minutes % 1440 / 60, minutes % 60);
    let parts = if d > 0 {
        vec![unit(d, "día", "días"), unit(h, "hora", "horas")]
    } else if h > 0 {
        vec![unit(h, "hora", "horas"), unit(m, "minuto", "minutos")]
    } else {
        vec![unit(m, "minuto", "minutos")]
    };
    parts.into_iter().filter(|p| !p.starts_with("0 ")).collect::<Vec<_>>().join(" y ")
}

pub fn explain(answer: &str) -> String {
    match answer {
        "ERR code" => "El código no es válido, ya se usó o caducó. Pide uno nuevo a un administrador.".into(),
        a if a.starts_with("ERR banned ") => banned(&a[11..]),
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

// ---- client improvements ----

/// "Mejoras del cliente" switched on: our version.dll in the game folder, set by grp-client.ini next to it (more
/// display modes in Options > Video, a frame cap other than 60). Switched off, neither file is there and the game is
/// the original.
#[derive(Clone, Copy)]
pub struct ClientMods {
    pub max_fps: Option<u32>, // None: the monitor's refresh rate (the DLL clamps it to 60-240)
}

const CLIENT_DLL: &str = "version.dll";
const CLIENT_INI: &str = "grp-client.ini";

/// The player's choice: on or off, and the frame cap as typed (a number; anything else is automatic).
pub fn client_mods(on: bool, max_fps: &str) -> Option<ClientMods> {
    on.then(|| ClientMods { max_fps: max_fps.trim().parse().ok() })
}

/// Brings a DLL in the game folder up to the one published next to the vault: client/<name>.txt holds
/// "<sha256> <size>" of client/<name>.dll (version.txt for version.dll, d3d9.txt for d3d9.dll). The file only ever
/// lands whole and checked (see download).
fn fetch_client_dll(vault_key: &str, dll: &Path) -> Result<(), String> {
    let name = dll.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let listing = format!("{}.txt", name.trim_end_matches(".dll"));
    let base = format!("{}/files/{}/client/", base_url(), vault_key);
    let text = get_text(&format!("{}{}", base, listing), 10)?;
    let mut v = text.split_whitespace();
    let (sha, size) = match (v.next(), v.next().and_then(|s| s.parse::<u64>().ok())) {
        (Some(sha), Some(size)) if sha.len() == 64 && sha.bytes().all(|b| b.is_ascii_hexdigit()) => (sha.to_lowercase(), size),
        _ => return Err(format!("bad client/{}: {}", listing, text.chars().take(80).collect::<String>())),
    };
    if sha256_file(dll).map(|h| h == sha).unwrap_or(false) {
        return Ok(());
    }
    download(&format!("{}{}", base, name), dll, size, &sha, &mut |_| {})?;
    log(&format!("client {} {} installed", name, &sha[..12]));
    Ok(())
}

/// Makes the game folder match the choice. On: the DLL (fetched when the sign-in gave a vault key) and its settings.
/// Off: removes both. A failed download is logged and the game starts anyway, with the DLL it had or none.
fn set_client_mods(vault_key: Option<&str>, game_dir: &Path, mods: Option<ClientMods>, progress: &dyn Fn(&str, f64)) {
    let (dll, ini) = (game_dir.join(CLIENT_DLL), game_dir.join(CLIENT_INI));
    let Some(m) = mods else {
        remove_files("client improvements off", &[dll.clone(), ini, PathBuf::from(format!("{}.part", dll.display()))]);
        return;
    };
    if let Some(key) = vault_key {
        progress("Comprobando las mejoras del cliente...", -1.0);
        if let Err(e) = fetch_client_dll(key, &dll) {
            log(&format!("client dll not updated: {}", e));
        }
    }
    let max_fps = m.max_fps.map(|n| n.to_string()).unwrap_or_default();
    let _ = std::fs::write(&ini, format!("[Client]\r\nMaxFps={}\r\nUnlockResolutions=1\r\n", max_fps));
    log(&format!("client improvements on: max fps {}", if max_fps.is_empty() { "auto" } else { &max_fps }));
}

/// Deletes files the launcher put in the game folder; one that is already gone is fine.
fn remove_files(why: &str, files: &[PathBuf]) {
    for f in files {
        match std::fs::remove_file(f) {
            Ok(()) => log(&format!("{}: removed {}", why, f.display())),
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => log(&format!("could not remove {}: {}", f.display(), e)),
            Err(_) => {}
        }
    }
}

// ---- Vulkan through DXVK ----
//
// "Vulkan (DXVK)" on Windows: DXVK's 32-bit d3d9.dll in the game folder, next to our version.dll, so the game draws
// through Vulkan as it does under Proton on Linux. It is placed only when the Vulkan check passes, and a game that
// never showed a frame with it turns the option off. On Linux nothing is placed: Proton puts its own DXVK in the
// prefix, and the launcher only logs which d3d9.dll the game ran with.

/// Published next to the vault as client/d3d9.dll with client/d3d9.txt (see fetch_client_dll).
#[cfg(windows)]
const DXVK_DLL: &str = "d3d9.dll";
/// DXVK's zlib licence, written next to its DLL.
#[cfg(windows)]
const DXVK_LICENSE: &str = "DXVK-LICENSE.txt";
/// DXVK's own log, which it writes in the game's working directory (the game folder).
#[cfg(windows)]
const DXVK_LOG: &str = "GhostReconPhantoms_d3d9.log";
/// What DXVK logs each time it makes a swapchain, that is, once it is about to show frames.
#[cfg(any(windows, test))]
const DXVK_PRESENTING: &str = "Presenter: Actual swapchain properties";

#[cfg(windows)]
fn dxvk_files(game_dir: &Path) -> [PathBuf; 3] {
    [game_dir.join(DXVK_DLL), game_dir.join(format!("{}.part", DXVK_DLL)), game_dir.join(DXVK_LICENSE)]
}

/// Makes the game folder match the choice. On, with a Vulkan device DXVK accepts: its d3d9.dll (fetched when the
/// sign-in gave a vault key) and licence. Otherwise neither is there and the game uses Direct3D 9 as published.
#[cfg(windows)]
fn set_dxvk(vault_key: Option<&str>, game_dir: &Path, on: bool, progress: &dyn Fn(&str, f64)) {
    // A d3d9.dll that is not DXVK is the player's own (ReShade or the like): never overwritten nor removed.
    let dll = game_dir.join(DXVK_DLL);
    if dll.is_file() && !is_dxvk_file(&dll) {
        log(&format!("{} is not DXVK (the player's own?): left as it is, the launcher does not place DXVK", dll.display()));
        return;
    }
    if !on {
        remove_files("vulkan (dxvk) off", &dxvk_files(game_dir));
        return;
    }
    progress("Comprobando Vulkan...", -1.0);
    let device = match vulkan_ready() {
        Ok(device) => device,
        Err(why) => {
            log(&format!("dxvk not used, the game runs on Direct3D 9: {}", why));
            remove_files("dxvk not used", &dxvk_files(game_dir));
            return;
        }
    };
    log(&format!("vulkan ok: {}", device));
    let dll = game_dir.join(DXVK_DLL);
    if let Some(key) = vault_key {
        if let Err(e) = fetch_client_dll(key, &dll) {
            log(&format!("dxvk not updated: {}", e));
        }
    }
    if dll.is_file() {
        let _ = std::fs::write(game_dir.join(DXVK_LICENSE), include_str!("../../third-party/DXVK-LICENSE.txt"));
        log("dxvk on: the game runs on Vulkan");
    } else {
        log("dxvk on but there is no d3d9.dll: the game runs on Direct3D 9");
    }
}

/// Whether DXVK can run here, decided out of this process: a broken driver or Vulkan layer can crash or hang the
/// process that loads it, so `GRPLauncher.exe --vulkan-check` (vulkan_probe) does it and this waits up to 20 s.
/// Also needs the 32-bit Vulkan loader the game will load. GRP_VULKAN_CHECK=pass|fail decides instead (tests).
#[cfg(windows)]
fn vulkan_ready() -> Result<String, String> {
    match std::env::var("GRP_VULKAN_CHECK").as_deref() {
        Ok("pass") => return Ok("not checked (GRP_VULKAN_CHECK=pass)".into()),
        Ok("fail") => return Err("not checked (GRP_VULKAN_CHECK=fail)".into()),
        _ => {}
    }
    let loader = windows_dir().join("SysWOW64").join("vulkan-1.dll");
    if !loader.is_file() {
        return Err(format!("no 32-bit Vulkan loader ({})", loader.display()));
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut child = std::process::Command::new(exe)
        .arg("--vulkan-check")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("the Vulkan check did not start: {}", e))?;
    let start = std::time::Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        if start.elapsed() > Duration::from_secs(20) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("the Vulkan check did not finish in 20 s".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let mut out = String::new();
    if let Some(mut o) = child.stdout.take() {
        let _ = o.read_to_string(&mut out);
    }
    let out = out.trim();
    match out.strip_prefix("ok ") {
        Some(device) if status.success() => Ok(device.to_string()),
        _ if out.is_empty() => Err(format!("the Vulkan check failed ({})", status)),
        _ => Err(out.trim_start_matches("no ").to_string()),
    }
}

/// Vulkan 1.3, the version DXVK asks of the driver (DxvkVulkanApiVersion).
#[cfg(any(windows, test))]
const VK_API_1_3: u32 = (1 << 22) | (3 << 12);
/// The device extensions DXVK 3.1.1 refuses to run without (featureRequired in dxvk_device_info.cpp).
#[cfg(any(windows, test))]
const DXVK_EXTENSIONS: [&str; 6] =
    ["VK_KHR_swapchain", "VK_EXT_robustness2", "VK_KHR_maintenance5", "VK_KHR_maintenance6", "VK_KHR_load_store_op_none", "VK_EXT_depth_clip_enable"];
#[cfg(any(windows, test))]
const VK_DEVICE_TYPE_CPU: i32 = 4;

#[cfg(any(windows, test))]
fn vk_version(v: u32) -> String {
    format!("{}.{}.{}", (v >> 22) & 0x7f, (v >> 12) & 0x3ff, v & 0xfff)
}

/// Why DXVK would skip a Vulkan device, as DXVK's own device filter decides; None when it would use it.
#[cfg(any(windows, test))]
fn dxvk_device_problem(api_version: u32, device_type: i32, extensions: &[String]) -> Option<String> {
    if device_type == VK_DEVICE_TYPE_CPU {
        return Some("software renderer".into());
    }
    if api_version < VK_API_1_3 {
        return Some(format!("Vulkan {}, DXVK needs 1.3", vk_version(api_version)));
    }
    let missing: Vec<&str> = DXVK_EXTENSIONS.into_iter().filter(|e| !extensions.iter().any(|x| x == e)).collect();
    (!missing.is_empty()).then(|| format!("no {}", missing.join(", ")))
}

/// What `GRPLauncher.exe --vulkan-check` runs: the first Vulkan device DXVK would use ("<name> (Vulkan x.y.z)"), or
/// why there is none (no driver, an old loader, each device and why DXVK would skip it).
#[cfg(windows)]
pub fn vulkan_probe() -> Result<String, String> {
    use std::ffi::{c_char, c_void, CStr};
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
    type Handle = *mut c_void; // VkInstance, VkPhysicalDevice
    #[repr(C)]
    struct AppInfo {
        s_type: i32,
        next: *const c_void,
        app_name: *const c_char,
        app_version: u32,
        engine_name: *const c_char,
        engine_version: u32,
        api_version: u32,
    }
    #[repr(C)]
    struct InstanceInfo {
        s_type: i32,
        next: *const c_void,
        flags: u32,
        app: *const AppInfo,
        layer_count: u32,
        layers: *const *const c_char,
        extension_count: u32,
        extensions: *const *const c_char,
    }
    #[repr(C)]
    struct ExtensionProperties {
        name: [c_char; 256],
        spec_version: u32,
    }
    type GetInstanceProcAddr = unsafe extern "system" fn(Handle, *const c_char) -> *const c_void;
    type EnumerateInstanceVersion = unsafe extern "system" fn(*mut u32) -> i32;
    type CreateInstance = unsafe extern "system" fn(*const InstanceInfo, *const c_void, *mut Handle) -> i32;
    type DestroyInstance = unsafe extern "system" fn(Handle, *const c_void);
    type EnumeratePhysicalDevices = unsafe extern "system" fn(Handle, *mut u32, *mut Handle) -> i32;
    // VkPhysicalDeviceProperties is 824 bytes: apiVersion at 0, deviceType at 16, deviceName (256) at 20
    type GetPhysicalDeviceProperties = unsafe extern "system" fn(Handle, *mut u64);
    type EnumerateDeviceExtensionProperties = unsafe extern "system" fn(Handle, *const c_char, *mut u32, *mut ExtensionProperties) -> i32;

    let name = |raw: &[c_char]| unsafe { CStr::from_ptr(raw.as_ptr()) }.to_string_lossy().into_owned();
    unsafe {
        let wide: Vec<u16> = "vulkan-1.dll".encode_utf16().chain(Some(0)).collect();
        let lib = LoadLibraryW(wide.as_ptr());
        if lib.is_null() {
            return Err("no Vulkan driver (vulkan-1.dll did not load)".into());
        }
        let gipa: GetInstanceProcAddr = match GetProcAddress(lib, c"vkGetInstanceProcAddr".as_ptr().cast()) {
            Some(f) => std::mem::transmute(f),
            None => return Err("vulkan-1.dll has no vkGetInstanceProcAddr".into()),
        };
        let get = |instance: Handle, f: &CStr| -> Result<*const c_void, String> {
            let p = gipa(instance, f.as_ptr());
            if p.is_null() {
                Err(format!("the Vulkan loader has no {}", f.to_string_lossy()))
            } else {
                Ok(p)
            }
        };
        let enumerate_version: EnumerateInstanceVersion = std::mem::transmute(get(null_mut(), c"vkEnumerateInstanceVersion").map_err(|_| "Vulkan 1.0 loader, DXVK needs 1.3".to_string())?);
        let mut loader = 0u32;
        enumerate_version(&mut loader);
        if loader < VK_API_1_3 {
            return Err(format!("Vulkan {} loader, DXVK needs 1.3", vk_version(loader)));
        }
        let app = AppInfo { s_type: 0, next: null(), app_name: c"GRPLauncher".as_ptr(), app_version: 0, engine_name: null(), engine_version: 0, api_version: VK_API_1_3 };
        let info = InstanceInfo { s_type: 1, next: null(), flags: 0, app: &app, layer_count: 0, layers: null(), extension_count: 0, extensions: null() };
        let create: CreateInstance = std::mem::transmute(get(null_mut(), c"vkCreateInstance")?);
        let mut instance: Handle = null_mut();
        let r = create(&info, null(), &mut instance);
        if r != 0 {
            return Err(format!("vkCreateInstance failed ({})", r));
        }
        let result = (|| -> Result<String, String> {
            let enumerate: EnumeratePhysicalDevices = std::mem::transmute(get(instance, c"vkEnumeratePhysicalDevices")?);
            let properties: GetPhysicalDeviceProperties = std::mem::transmute(get(instance, c"vkGetPhysicalDeviceProperties")?);
            let extensions_of: EnumerateDeviceExtensionProperties = std::mem::transmute(get(instance, c"vkEnumerateDeviceExtensionProperties")?);
            let mut count = 0u32;
            enumerate(instance, &mut count, null_mut());
            let mut devices: Vec<Handle> = vec![null_mut(); count as usize];
            enumerate(instance, &mut count, devices.as_mut_ptr());
            devices.truncate(count as usize);
            let mut skipped = Vec::new();
            for d in devices {
                let mut p = [0u64; 128];
                properties(d, p.as_mut_ptr());
                let bytes = std::slice::from_raw_parts(p.as_ptr().cast::<u8>(), 1024);
                let api = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
                let kind = i32::from_le_bytes(bytes[16..20].try_into().unwrap());
                let device = format!("{} (Vulkan {})", name(std::slice::from_raw_parts(bytes[20..].as_ptr().cast::<c_char>(), 256)), vk_version(api));
                let mut n = 0u32;
                extensions_of(d, null(), &mut n, null_mut());
                let mut list: Vec<ExtensionProperties> = (0..n).map(|_| ExtensionProperties { name: [0; 256], spec_version: 0 }).collect();
                extensions_of(d, null(), &mut n, list.as_mut_ptr());
                let names: Vec<String> = list.iter().take(n as usize).map(|e| name(&e.name)).collect();
                match dxvk_device_problem(api, kind, &names) {
                    None => return Ok(device),
                    Some(why) => skipped.push(format!("{}: {}", device, why)),
                }
            }
            Err(if skipped.is_empty() { "no Vulkan device".into() } else { skipped.join("; ") })
        })();
        if let Ok(destroy) = get(instance, c"vkDestroyInstance") {
            std::mem::transmute::<*const c_void, DestroyInstance>(destroy)(instance, null());
        }
        result
    }
}

/// After the game ran with DXVK: None when DXVK got to show frames, otherwise why the start is taken as failed (its
/// first error line, or no log at all: the DLL never loaded or the game ended before drawing anything).
#[cfg(any(windows, test))]
fn dxvk_failure(dxvk_log: Option<&str>) -> Option<String> {
    let Some(text) = dxvk_log else { return Some("DXVK wrote no log".into()) };
    if text.contains(DXVK_PRESENTING) {
        return None;
    }
    Some(text.lines().find(|l| l.starts_with("err:")).map(|l| l.trim().to_string()).unwrap_or_else(|| "DXVK never showed a frame".into()))
}

#[cfg(windows)]
const DXVK_FAILED: &str = "El juego no pudo abrirse con Vulkan (DXVK) en este equipo. Desactivamos la opción «Vulkan (DXVK)» en \
Opciones, así que la próxima vez se abrirá con DirectX 9. Pulsa JUGAR otra vez.";

/// Which d3d9.dll Proton put in the prefix for the 32-bit game: DXVK (Vulkan), as expected, or Wine's own WineD3D
/// (PROTON_USE_WINED3D, or a broken prefix).
#[cfg(not(windows))]
fn prefix_d3d9(prefix: &Path) -> String {
    let file = prefix.join("drive_c").join("windows").join("syswow64").join("d3d9.dll");
    let kind = std::fs::read(&file).map(|b| d3d9_kind(&b).to_string()).unwrap_or_else(|e| format!("unreadable ({})", e));
    let wined3d = std::env::var("PROTON_USE_WINED3D").map(|v| format!(", PROTON_USE_WINED3D={}", v)).unwrap_or_default();
    format!("{} ({}){}", kind, file.display(), wined3d)
}

/// Whether a d3d9.dll on disk is DXVK's (an unreadable one is not).
#[cfg(windows)]
fn is_dxvk_file(dll: &Path) -> bool {
    std::fs::read(dll).map(|b| d3d9_kind(&b) == D3D9_DXVK).unwrap_or(false)
}

const D3D9_DXVK: &str = "DXVK (Vulkan)";

fn d3d9_kind(dll: &[u8]) -> &'static str {
    let has = |s: &[u8]| dll.windows(s.len()).any(|w| w == s);
    if has(b"Wine builtin DLL") {
        "WineD3D (Wine's own d3d9, not DXVK)"
    } else if has(b"dxvk") {
        D3D9_DXVK
    } else {
        "unknown d3d9.dll"
    }
}

// ---- the game ----

pub struct PlayOptions {
    pub sync: bool,
    pub width: u32,  // 0: leave the game's resolution as it is
    pub height: u32,
    pub windowed: bool,
    pub client: Option<ClientMods>, // None: client improvements off
    #[cfg_attr(not(windows), allow(dead_code))]
    pub dxvk: bool,                 // "Vulkan (DXVK)"; Windows only, Linux always has Proton's
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
    // On Linux the game runs under Wine on XWayland, where exclusive fullscreen leaves the mouse off from what is drawn:
    // a window the size of the screen looks the same and the clicks land where they should.
    let o = &PlayOptions { windowed: o.windowed || cfg!(target_os = "linux"), ..*o };
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
    let vault_key = if o.sync { a.get(3).copied() } else { None };
    set_client_mods(vault_key, game_dir, o.client, progress);
    #[cfg(windows)]
    set_dxvk(vault_key, game_dir, o.dxvk, progress);
    run_game(game_dir, &format!("{}.{}", a[1], a[2]), progress, playing)
}

/// The 32-bit DirectX 9 DLLs of the June 2010 runtime that the game needs and Windows 10 and 11 do not ship.
/// Wine and Proton prefixes carry their own, so under Wine they are always there.
#[cfg(windows)]
const DX_DLLS: [&str; 2] = ["d3dx9_43.dll", "xinput1_3.dll"];

/// Where 32-bit system DLLs live: SysWOW64 on 64-bit Windows, System32 on 32-bit. GRP_DX_CHECK names another folder
/// to look in instead (tests: an empty folder makes the launcher install DirectX again).
#[cfg(windows)]
fn dx_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("GRP_DX_CHECK") {
        return PathBuf::from(dir);
    }
    let windows = windows_dir();
    let wow = windows.join("SysWOW64");
    if wow.is_dir() {
        wow
    } else {
        windows.join("System32")
    }
}

#[cfg(windows)]
fn windows_dir() -> PathBuf {
    std::env::var_os("WINDIR").or_else(|| std::env::var_os("SystemRoot")).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
}

/// Installs the DirectX 9 runtime from the game folder's DXRedist when one of DX_DLLS is missing: DXSETUP.exe /silent,
/// elevated (one UAC prompt), waited for. A run that succeeded is not repeated while the launcher stays open.
#[cfg(windows)]
fn ensure_directx(game_dir: &Path, progress: &dyn Fn(&str, f64)) -> Result<(), String> {
    use std::sync::atomic::{AtomicBool, Ordering};
    static INSTALLED: AtomicBool = AtomicBool::new(false);
    let dir = dx_dir();
    let missing: Vec<&str> = DX_DLLS.into_iter().filter(|f| !dir.join(f).is_file()).collect();
    if missing.is_empty() || INSTALLED.load(Ordering::Relaxed) {
        return Ok(());
    }
    let setup = game_dir.join("DXRedist").join("DXSETUP.exe");
    log(&format!("directx: {} missing in {}", missing.join(", "), dir.display()));
    if !setup.is_file() {
        return Err(format!("Falta DirectX 9 ({}) y su instalador no está en la carpeta del juego:\n{}", missing.join(", "), setup.display()));
    }
    progress("Instalando DirectX 9 (solo la primera vez)... Si Windows ofrece instalar .NET Framework 3.5, puedes omitirlo.", -1.0);
    let by_hand = format!("Para instalarlo a mano, ejecuta DXSETUP.exe de la carpeta DXRedist del juego y vuelve a pulsar Jugar:\n{}", setup.display());
    match run_elevated(&setup, "/silent") {
        Ok(0) => {
            log("directx: DXSETUP finished");
            INSTALLED.store(true, Ordering::Relaxed);
            Ok(())
        }
        Ok(code) => {
            log(&format!("directx: DXSETUP exit code {}", code));
            Err(format!("La instalación de DirectX 9 falló (código {}). El juego no puede abrirse sin él.\n{}", code as i32, by_hand))
        }
        Err(windows_sys::Win32::Foundation::ERROR_CANCELLED) => {
            log("directx: UAC prompt declined");
            Err(format!("El juego necesita DirectX 9 y su instalación pide permiso de administrador. Vuelve a pulsar Jugar y acepta el aviso de Windows.\n{}", by_hand))
        }
        Err(e) => {
            log(&format!("directx: DXSETUP did not start, error {}", e));
            Err(format!("No se pudo abrir el instalador de DirectX 9 (error {}).\n{}", e, by_hand))
        }
    }
}

/// Runs a program as administrator and waits for it: its exit code, or the Windows error when it did not start
/// (ERROR_CANCELLED: the UAC prompt was declined).
#[cfg(windows)]
fn run_elevated(exe: &Path, args: &str) -> Result<u32, u32> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError};
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject, INFINITE};
    use windows_sys::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let wide = |s: &std::ffi::OsStr| s.encode_wide().chain(Some(0)).collect::<Vec<u16>>();
    let verb = wide("runas".as_ref());
    let file = wide(exe.as_os_str());
    let params = wide(args.as_ref());
    let dir = wide(exe.parent().unwrap_or(exe).as_os_str());
    unsafe {
        let mut info: SHELLEXECUTEINFOW = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC;
        info.lpVerb = verb.as_ptr();
        info.lpFile = file.as_ptr();
        info.lpParameters = params.as_ptr();
        info.lpDirectory = dir.as_ptr();
        info.nShow = SW_SHOWNORMAL;
        if ShellExecuteExW(&mut info) == 0 {
            return Err(GetLastError());
        }
        if info.hProcess.is_null() {
            return Err(0);
        }
        WaitForSingleObject(info.hProcess, INFINITE);
        let mut code = 1u32;
        GetExitCodeProcess(info.hProcess, &mut code);
        CloseHandle(info.hProcess);
        Ok(code)
    }
}

/// Runs the game with the session already made ("<name>.<token>") and waits for it to exit.
#[cfg(windows)]
pub fn run_game(game_dir: &Path, session: &str, progress: &dyn Fn(&str, f64), playing: &dyn Fn()) -> Result<(), String> {
    ensure_directx(game_dir, progress)?;
    let dxvk = is_dxvk_file(&game_dir.join(DXVK_DLL));
    let dxvk_log = game_dir.join(DXVK_LOG);
    if dxvk {
        let _ = std::fs::remove_file(&dxvk_log); // only this run's log counts
    }
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
    log(&format!("game started, pid {}, on {}", child.id(), if dxvk { "Vulkan (DXVK)" } else { "Direct3D 9" }));
    progress("Jugando. Esta ventana vuelve cuando cierres el juego.", -1.0);
    playing();
    let code = child.wait().map(|s| s.code().unwrap_or(-1)).unwrap_or(-1);
    log(&format!("game exited, code {}", code));
    if dxvk {
        let text = std::fs::read(&dxvk_log).ok().map(|b| String::from_utf8_lossy(&b).into_owned());
        if let Some(why) = dxvk_failure(text.as_deref()) {
            log(&format!("dxvk failed ({}): switched off, the next start uses Direct3D 9", why));
            remove_files("dxvk failed", &dxvk_files(game_dir));
            save_settings(&[("dxvk", "0".into())]);
            return Err(DXVK_FAILED.into());
        }
    }
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
        // as on Windows: a version.dll in the game folder (ours, with client improvements on) wins over Wine's own
        .env("WINEDLLOVERRIDES", "version=n,b")
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
    let running = std::sync::atomic::AtomicBool::new(true);
    let code = std::thread::scope(|s| {
        if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some() {
            s.spawn(|| fullscreen_on_hyprland(&running));
        }
        let code = child.wait().map(|s| s.code().unwrap_or(-1)).unwrap_or(-1);
        running.store(false, std::sync::atomic::Ordering::Relaxed);
        code
    });
    log(&format!("umu-run exited, code {}", code));
    log(&format!("d3d9 in the prefix: {}", prefix_d3d9(&prefix)));
    Ok(())
}

/// Hyprland tiles the game's window under its bar. Each new game window is made fullscreen there once, so the player
/// can still take it out of fullscreen. The game itself stays windowed (see set_resolution).
#[cfg(not(windows))]
fn fullscreen_on_hyprland(running: &std::sync::atomic::AtomicBool) {
    let hyprctl = |args: &[&str]| std::process::Command::new("hyprctl").args(args).output().ok();
    let mut seen = std::collections::HashSet::new();
    while running.load(std::sync::atomic::Ordering::Relaxed) {
        std::thread::sleep(Duration::from_secs(1));
        let Some(out) = hyprctl(&["clients", "-j"]) else { return };
        let clients: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap_or_default();
        for c in clients.iter().filter(|c| c["title"] == "Ghost Recon Phantoms" && c["xwayland"] == true) {
            let Some(address) = c["address"].as_str() else { continue };
            if !seen.insert(address.to_string()) || c["fullscreen"] != 0 {
                continue;
            }
            // Hyprland with the Lua config takes Lua dispatchers; older versions the classic form
            let lua = format!("hl.dsp.window.fullscreen({{ mode = \"fullscreen\", action = \"set\", window = \"address:{}\" }})", address);
            let classic = format!("dispatch focuswindow address:{}; dispatch fullscreenstate 2 2", address);
            let ok = |o: Option<std::process::Output>| o.map(|o| o.status.success()).unwrap_or(false);
            let done = ok(hyprctl(&["dispatch", &lua])) || ok(hyprctl(&["--batch", &classic]));
            log(&format!("hyprland fullscreen {}: {}", address, if done { "ok" } else { "failed" }));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn dxvk_takes_a_vulkan_1_3_gpu_with_its_extensions() {
        assert_eq!(dxvk_device_problem(VK_API_1_3 + 280, 2, &names(&DXVK_EXTENSIONS)), None);
    }

    #[test]
    fn dxvk_skips_old_drivers_software_renderers_and_missing_extensions() {
        let all = names(&DXVK_EXTENSIONS);
        assert_eq!(dxvk_device_problem((1 << 22) | (2 << 12) | 203, 2, &all).unwrap(), "Vulkan 1.2.203, DXVK needs 1.3");
        assert_eq!(dxvk_device_problem(VK_API_1_3, VK_DEVICE_TYPE_CPU, &all).unwrap(), "software renderer");
        let without = names(&["VK_KHR_swapchain", "VK_EXT_robustness2", "VK_KHR_maintenance5", "VK_KHR_load_store_op_none", "VK_EXT_depth_clip_enable"]);
        assert_eq!(dxvk_device_problem(VK_API_1_3, 1, &without).unwrap(), "no VK_KHR_maintenance6");
    }

    #[test]
    fn a_run_counts_as_working_once_dxvk_presented() {
        let log = "info:  DXVK: v3.1.1\ninfo:  Presenter: Actual swapchain properties:\ninfo:    Format:       VK_FORMAT_B8G8R8A8_UNORM\n";
        assert_eq!(dxvk_failure(Some(log)), None);
    }

    #[test]
    fn a_run_without_a_frame_counts_as_failed() {
        assert_eq!(dxvk_failure(None).unwrap(), "DXVK wrote no log");
        let log = "info:  Game: GhostReconPhantoms.exe\ninfo:  DXVK: v3.1.1\nerr:   Failed to load vulkan-1 library.\n";
        assert_eq!(dxvk_failure(Some(log)).unwrap(), "err:   Failed to load vulkan-1 library.");
        assert_eq!(dxvk_failure(Some("info:  DXVK: v3.1.1\n")).unwrap(), "DXVK never showed a frame");
    }

    #[test]
    fn tells_dxvk_from_wined3d() {
        assert_eq!(d3d9_kind(b"MZ\x90\0Wine builtin DLL\0..."), "WineD3D (Wine's own d3d9, not DXVK)");
        assert_eq!(d3d9_kind(b"MZ\x90\0...dxvk-cache..."), "DXVK (Vulkan)");
        assert_eq!(d3d9_kind(b"MZ\x90\0"), "unknown d3d9.dll");
    }
}
