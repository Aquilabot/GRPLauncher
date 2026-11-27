// Voice chat (issue #26). The community server runs a Mumble server next to the game servers; the launcher does not
// carry voice itself: it opens the player's own Mumble client on it, signed in with the account's name
// (mumble://<name>@<host>:<port>/<channel>), when the player asks or, if they chose so, when the game starts.
//
// Where the server is, first one that answers: voicehost / voiceport of launcher.ini; GET /voice of the account
// service ("OK mumble <host> <port> <channel>"); the game server's own name on Mumble's port 64738.

use crate::core;
use serde::Serialize;

pub const DEFAULT_PORT: u16 = 64738;
pub const DEFAULT_CHANNEL: &str = "Lobby";
/// The name the community server's Mumble certificate is for.
const HOME_HOST: &str = "grp.limacashout.pro";
const HOME_SERVER: &str = "56.127.0.17";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Server {
    pub host: String,
    pub port: u16,
    pub channel: String,
}

/// A host name or address fit for a link: letters, digits, dots and hyphens, not starting with a hyphen or a dot.
/// What the network says goes into a command line, so anything else is refused.
pub fn clean_host(h: &str) -> Option<String> {
    let h = h.trim();
    let ok = !h.is_empty()
        && h.len() <= 253
        && !h.starts_with(['-', '.'])
        && !h.ends_with(['-', '.'])
        && h.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.');
    ok.then(|| h.to_string())
}

/// A channel name fit for a link: letters, digits, spaces, hyphens and underscores.
pub fn clean_channel(c: &str) -> Option<String> {
    let c = c.trim();
    let ok = !c.is_empty() && c.len() <= 64 && c.bytes().all(|b| b.is_ascii_alphanumeric() || b == b' ' || b == b'-' || b == b'_');
    ok.then(|| c.to_string())
}

/// The answer of GET /voice: "OK mumble <host> <port> <channel>".
pub fn parse_info(line: &str) -> Option<Server> {
    let mut w = line.split_whitespace();
    if w.next()? != "OK" || w.next()? != "mumble" {
        return None;
    }
    let host = clean_host(w.next()?)?;
    let port: u16 = w.next()?.parse().ok().filter(|p| *p != 0)?;
    let channel = clean_channel(&w.collect::<Vec<_>>().join(" "))?;
    Some(Server { host, port, channel })
}

/// The server when nothing is configured and nobody answers: the community server's own name on Mumble's port.
fn fallback(server: &str) -> Server {
    let host = if server == HOME_SERVER { HOME_HOST.to_string() } else { clean_host(server).unwrap_or_else(|| HOME_HOST.to_string()) };
    Server { host, port: DEFAULT_PORT, channel: DEFAULT_CHANNEL.to_string() }
}

/// Where to join: see the top of this file.
pub fn server() -> Server {
    let c = core::config();
    let asked = core::get_text(&format!("{}/voice", core::base_url()), 5).ok().and_then(|t| parse_info(t.trim()));
    let mut s = asked.unwrap_or_else(|| fallback(&c.server));
    if let Some(h) = clean_host(&c.voice_host) {
        s.host = h;
    }
    if c.voice_port != 0 {
        s.port = c.voice_port;
    }
    s
}

fn escape(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' { (b as char).to_string() } else { format!("%{:02X}", b) }).collect()
}

/// mumble://<name>@<host>:<port>/<channel>?version=1.2.0 (Mumble's link format; the name and channel are escaped).
pub fn url(name: &str, s: &Server) -> String {
    format!("mumble://{}@{}:{}/{}?version=1.2.0", escape(name.trim()), s.host, s.port, escape(&s.channel))
}

/// What `xdg-mime query default x-scheme-handler/mumble` printed: a desktop file when some program handles mumble://.
pub fn handler_registered(xdg_mime_output: &str) -> bool {
    xdg_mime_output.trim().ends_with(".desktop")
}

/// Is there a program that opens mumble:// links? Windows: Mumble's installer registers the scheme.
pub fn installed() -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("reg")
            .args(["query", r"HKCR\mumble\shell\open\command"])
            .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("xdg-mime")
            .args(["query", "default", "x-scheme-handler/mumble"])
            .output()
            .map(|o| handler_registered(&String::from_utf8_lossy(&o.stdout)))
            .unwrap_or(false)
    }
}

#[derive(Serialize, Clone)]
pub struct Joined {
    /// "opened": the link went to the player's Mumble; "missing": no Mumble here; "error": it did not open.
    pub status: &'static str,
    pub host: String,
    pub port: u16,
    pub channel: String,
}

/// Opens the player's Mumble on the server. The password is never involved: the Mumble server admits by name and
/// the name is the account's.
pub fn join(name: &str) -> Joined {
    let s = server();
    let result = |status| Joined { status, host: s.host.clone(), port: s.port, channel: s.channel.clone() };
    if name.trim().is_empty() {
        return result("error");
    }
    if !installed() {
        core::log("voice: no program opens mumble:// links");
        return result("missing");
    }
    let link = url(name, &s);
    #[cfg(windows)]
    let r = std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", &link]).spawn();
    #[cfg(not(windows))]
    let r = std::process::Command::new("xdg-open").arg(&link).spawn();
    match r {
        Ok(_) => {
            core::log(&format!("voice: Mumble opened on {}:{} ({})", s.host, s.port, s.channel));
            result("opened")
        }
        Err(e) => {
            core::log(&format!("voice: Mumble not opened: {}", e));
            result("error")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_info_line_is_read() {
        assert_eq!(parse_info("OK mumble grp.limacashout.pro 64738 Lobby"), Some(Server { host: "grp.limacashout.pro".into(), port: 64738, channel: "Lobby".into() }));
        assert_eq!(parse_info("OK mumble voice.example.org 64739 Team Alpha").unwrap().channel, "Team Alpha");
        assert_eq!(parse_info("ERR off"), None);
        assert_eq!(parse_info("OK mumble host"), None);
        assert_eq!(parse_info("OK mumble host 0 Lobby"), None);
        assert_eq!(parse_info("OK mumble host 70000 Lobby"), None);
        assert_eq!(parse_info("<html>nope</html>"), None);
    }

    #[test]
    fn nothing_odd_from_the_network_reaches_the_command_line() {
        assert_eq!(parse_info("OK mumble evil.com/x&calc 64738 Lobby"), None);
        assert_eq!(parse_info("OK mumble host 64738 Lobby&calc"), None);
        assert_eq!(parse_info("OK mumble -oProxy=x 64738 Lobby"), None);
        assert_eq!(clean_host("a b"), None);
        assert_eq!(clean_host(""), None);
        assert_eq!(clean_host(".x"), None);
        assert_eq!(clean_host("203.0.113.7"), Some("203.0.113.7".into()));
    }

    #[test]
    fn the_link_carries_the_account_name_and_the_channel() {
        let s = Server { host: "grp.limacashout.pro".into(), port: 64738, channel: "Lobby".into() };
        assert_eq!(url("Tester", &s), "mumble://Tester@grp.limacashout.pro:64738/Lobby?version=1.2.0");
        assert_eq!(url(" a b@c ", &s), "mumble://a%20b%40c@grp.limacashout.pro:64738/Lobby?version=1.2.0");
        let team = Server { channel: "Team Alpha".into(), ..s };
        assert!(url("x", &team).contains("/Team%20Alpha?"));
    }

    #[test]
    fn without_a_name_for_the_server_the_community_one_is_used() {
        assert_eq!(fallback("56.127.0.17").host, "grp.limacashout.pro");
        assert_eq!(fallback("10.0.0.5").host, "10.0.0.5");
        assert_eq!(fallback("bad host!").host, "grp.limacashout.pro");
        assert_eq!(fallback("grp.limacashout.pro").port, 64738);
    }

    #[test]
    fn a_registered_handler_is_a_desktop_file() {
        assert!(handler_registered("info.mumble.Mumble.desktop\n"));
        assert!(handler_registered("mumble.desktop"));
        assert!(!handler_registered(""));
        assert!(!handler_registered("\n"));
    }
}
