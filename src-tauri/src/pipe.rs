// The pipe the game's own launcher protocol runs on, \\.\pipe\Ghost_Recon_Phantoms.
// Messages are a sequence of [u32 byte length][UTF-16LE text]; the game sends ["v2"][command].
// Answers are binary: a status byte 01 followed by the value; only notifyclientstart answers with a framed string.
use std::sync::{Mutex, OnceLock};

static USER: OnceLock<Mutex<String>> = OnceLock::new();

fn user() -> String {
    USER.get_or_init(|| Mutex::new(String::new())).lock().unwrap().clone()
}

fn frame(parts: &[&str]) -> Vec<u8> {
    let mut out = Vec::new();
    for p in parts {
        let b: Vec<u8> = p.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        out.extend_from_slice(&(b.len() as u32).to_le_bytes());
        out.extend_from_slice(&b);
    }
    out
}

fn unframe(m: &[u8]) -> Vec<String> {
    let mut parts = Vec::new();
    let mut o = 0;
    while o + 4 <= m.len() {
        let len = u32::from_le_bytes([m[o], m[o + 1], m[o + 2], m[o + 3]]) as usize;
        o += 4;
        if o + len > m.len() {
            break;
        }
        let u: Vec<u16> = m[o..o + len].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        parts.push(String::from_utf16_lossy(&u));
        o += len;
    }
    parts
}

fn reply(request: &[String]) -> Option<Vec<u8>> {
    if request.len() < 2 {
        return None;
    }
    let ok = |rest: Vec<u8>| {
        let mut v = vec![1u8];
        v.extend(rest);
        Some(v)
    };
    match request[1].as_str() {
        "epochnow" => {
            let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as i64;
            ok(t.to_le_bytes().to_vec())
        }
        "validate" => ok(vec![]),
        "notifyclientstart" => Some(frame(&["ok"])),
        "showeula" => Some(vec![1, 0]),
        "getdistributionsource" => ok(frame(&[""])),
        // the server checks the session token inside the user string; the password field is not used
        "getlogininfo" => ok(frame(&[&user(), "grp"])),
        "getproxy" => Some(vec![1, 0, 0, 0, 0, 0]),
        _ => ok(vec![]),
    }
}

/// Starts answering the game (once); later calls only change the user string handed to it.
pub fn start(game_user: String) {
    *USER.get_or_init(|| Mutex::new(String::new())).lock().unwrap() = game_user;
    static STARTED: OnceLock<()> = OnceLock::new();
    if STARTED.set(()).is_err() {
        return;
    }
    // The game fires several requests back to back, each on a new connection: several instances listen.
    for _ in 0..4 {
        std::thread::spawn(serve);
    }
}

#[cfg(not(windows))]
fn serve() {
    let _ = (reply(&[]), unframe(&[]));
}

#[cfg(windows)]
fn serve() {
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{ReadFile, WriteFile};
    use windows_sys::Win32::System::Pipes::{ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe};
    const PIPE_ACCESS_DUPLEX: u32 = 3;
    const PIPE_TYPE_MESSAGE: u32 = 4;
    const PIPE_READMODE_MESSAGE: u32 = 2;
    const ERROR_PIPE_CONNECTED: u32 = 535;
    let name: Vec<u16> = r"\\.\pipe\Ghost_Recon_Phantoms".encode_utf16().chain(std::iter::once(0)).collect();
    loop {
        // The game switches the pipe to message read mode, so it has to be a message pipe.
        let h = unsafe { CreateNamedPipeW(name.as_ptr(), PIPE_ACCESS_DUPLEX, PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE, 255, 4096, 4096, 0, std::ptr::null()) };
        if h == INVALID_HANDLE_VALUE {
            crate::core::log(&format!("pipe: create failed {}", unsafe { GetLastError() }));
            std::thread::sleep(std::time::Duration::from_secs(1));
            continue;
        }
        let connected = unsafe { ConnectNamedPipe(h, std::ptr::null_mut()) } != 0 || unsafe { GetLastError() } == ERROR_PIPE_CONNECTED;
        if connected {
            let mut buf = [0u8; 4096];
            loop {
                let mut read = 0u32;
                let ok = unsafe { ReadFile(h, buf.as_mut_ptr() as _, buf.len() as u32, &mut read, std::ptr::null_mut()) };
                if ok == 0 || read == 0 {
                    break;
                }
                let parts = unframe(&buf[..read as usize]);
                crate::core::log(&format!("pipe: {}", parts.join(" ")));
                if let Some(r) = reply(&parts) {
                    let mut written = 0u32;
                    unsafe { WriteFile(h, r.as_ptr() as _, r.len() as u32, &mut written, std::ptr::null_mut()) };
                }
            }
            unsafe { DisconnectNamedPipe(h) };
        }
        unsafe { CloseHandle(h) };
    }
}
