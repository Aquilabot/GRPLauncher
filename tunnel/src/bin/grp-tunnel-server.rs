//! The server relay (issue #34): runs on the VPS next to the lobby and the match servers.
//!
//!   grp-tunnel-server [--listen 0.0.0.0:21035] [--backend 127.0.0.1] [--ports 21030-21034]
//!                     [--whois 127.0.0.1:21039] [--idle 90] [--per-address 8] [--reuse-after 120]
//!                     [--duplicate-up-to 0]
//!
//! Writes one line per session to stdout and a summary every 5 minutes. Stops on SIGTERM/SIGINT like any process
//! (the sessions die with it; client relays say HELLO again and their players reconnect).
//! The lobby asks it who stands behind a player address with GRP_TUNNEL=1 (QuazalWV/Tunnel.cs).

use grp_tunnel::server::{ServerConfig, ServerRelay};
use grp_tunnel::{parse_ports, Redundancy};
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

fn stamp() -> String {
    let s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let (d, r) = (s / 86400, s % 86400);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = d as i64 + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", year, month, day, r / 3600, r % 3600 / 60, r % 60)
}

fn usage(why: &str) -> ! {
    eprintln!("{}\nusage: grp-tunnel-server [--listen addr:port] [--backend ip] [--ports 21030-21034] [--whois addr:port|off] [--idle s] [--per-address n] [--reuse-after s] [--duplicate-up-to bytes]", why);
    std::process::exit(2);
}

fn main() {
    let mut cfg = ServerConfig::new("0.0.0.0:21035".parse().unwrap(), vec![21030, 21031, 21032, 21033, 21034]);
    cfg.whois = Some("127.0.0.1:21039".parse().unwrap());
    cfg.log = Arc::new(|s: &str| println!("{} [TUNNEL] {}", stamp(), s));
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        let value = || args.get(i + 1).cloned().unwrap_or_else(|| usage(&format!("{} needs a value", args[i])));
        let secs = |v: String| Duration::from_secs(v.parse().unwrap_or_else(|_| usage(&format!("bad number {:?}", v))));
        match args[i].as_str() {
            "--listen" => cfg.listen = value().parse::<SocketAddr>().unwrap_or_else(|e| usage(&e.to_string())),
            "--backend" => cfg.backend = value().parse::<Ipv4Addr>().unwrap_or_else(|e| usage(&e.to_string())),
            "--ports" => cfg.ports = parse_ports(&value()).unwrap_or_else(|e| usage(&e)),
            "--whois" => {
                let v = value();
                cfg.whois = if v == "off" { None } else { Some(v.parse().unwrap_or_else(|e: std::net::AddrParseError| usage(&e.to_string()))) };
            }
            "--idle" => cfg.idle_timeout = secs(value()),
            "--per-address" => cfg.max_per_address = value().parse().unwrap_or_else(|_| usage("bad --per-address")),
            "--reuse-after" => cfg.reuse_after = secs(value()),
            "--duplicate-up-to" => cfg.redundancy = Redundancy { duplicate_up_to: value().parse().unwrap_or_else(|_| usage("bad --duplicate-up-to")) },
            "-h" | "--help" => usage("the GRP tunnel's server relay"),
            other => usage(&format!("unknown argument {:?}", other)),
        }
        i += 2;
    }
    let relay = ServerRelay::start(cfg).unwrap_or_else(|e| {
        eprintln!("{} [TUNNEL] cannot start: {}", stamp(), e);
        std::process::exit(1);
    });
    loop {
        std::thread::sleep(Duration::from_secs(300));
        let s = relay.stats();
        println!(
            "{} [TUNNEL] {} open, {} sessions so far; up {} datagrams ({} bytes), down {} ({} bytes), {} dropped, {} oversize",
            stamp(),
            s.sessions_open,
            s.sessions,
            s.up_datagrams,
            s.up_bytes,
            s.down_datagrams,
            s.down_bytes,
            s.dropped,
            s.oversize
        );
    }
}
