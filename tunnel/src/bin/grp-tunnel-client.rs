//! A client relay on its own, without the launcher (the launcher runs the same code inside it: core.rs).
//! For the test scripts, and for trying the tunnel against a server by hand.
//!
//!   grp-tunnel-client --server host:port [--ports 21030-21034] [--offset 0] [--listen 127.0.0.1] [--label name]
//!                     [--duplicate-up-to 0]
//!
//! Listens on <listen>:<server port + offset> for each port; with --offset, several on one machine (each one a
//! player). Prints "ready <server>=<local>,..." once the session is made, then a line of counters every 30 s, and
//! runs until it is killed.

use grp_tunnel::client::{ClientConfig, ClientRelay};
use grp_tunnel::{parse_ports, Redundancy};
use std::net::{Ipv4Addr, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;

fn usage(why: &str) -> ! {
    eprintln!("{}\nusage: grp-tunnel-client --server host:port [--ports 21030-21034] [--offset n] [--listen ip] [--label name] [--duplicate-up-to bytes]", why);
    std::process::exit(2);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (mut server, mut ports, mut offset, mut listen, mut label, mut dup) = (None, vec![21030, 21031, 21032, 21033, 21034], 0i32, Ipv4Addr::LOCALHOST, String::new(), 0usize);
    let mut i = 0;
    while i < args.len() {
        let v = args.get(i + 1).cloned().unwrap_or_else(|| usage(&format!("{} needs a value", args[i])));
        match args[i].as_str() {
            "--server" => server = Some(v.to_socket_addrs().ok().and_then(|mut a| a.next()).unwrap_or_else(|| usage(&format!("cannot resolve {}", v)))),
            "--ports" => ports = parse_ports(&v).unwrap_or_else(|e| usage(&e)),
            "--offset" => offset = v.parse().unwrap_or_else(|_| usage("bad --offset")),
            "--listen" => listen = v.parse().unwrap_or_else(|_| usage("bad --listen")),
            "--label" => label = v,
            "--duplicate-up-to" => dup = v.parse().unwrap_or_else(|_| usage("bad --duplicate-up-to")),
            other => usage(&format!("unknown argument {:?}", other)),
        }
        i += 2;
    }
    let server = server.unwrap_or_else(|| usage("--server is needed"));
    let mut cfg = ClientConfig::new(server, &ports);
    cfg.ports = ports
        .iter()
        .map(|&p| {
            let local = p as i32 + offset;
            if !(1..=65535).contains(&local) {
                usage("--offset takes a port out of range");
            }
            (p, local as u16)
        })
        .collect();
    cfg.listen_ip = listen;
    cfg.label = label;
    cfg.redundancy = Redundancy { duplicate_up_to: dup };
    cfg.log = Arc::new(|s: &str| println!("{}", s));
    let relay = ClientRelay::start(cfg).unwrap_or_else(|e| {
        eprintln!("tunnel failed: {}", e);
        std::process::exit(1);
    });
    println!("ready {}", ports.iter().map(|&p| format!("{}={}", p, relay.local_port(p).unwrap())).collect::<Vec<_>>().join(","));
    loop {
        std::thread::sleep(Duration::from_secs(30));
        let s = relay.stats();
        println!(
            "session {:08x}: up {} ({} bytes, largest {}), down {} ({} bytes, largest {}), dropped {}, oversize {}, sessions {}, rtt {} us",
            relay.session(),
            s.up_datagrams,
            s.up_bytes,
            s.up_largest,
            s.down_datagrams,
            s.down_bytes,
            s.down_largest,
            s.dropped,
            s.oversize,
            s.sessions,
            s.rtt_us
        );
    }
}
