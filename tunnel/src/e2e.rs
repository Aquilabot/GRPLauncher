//! Game -> client relay -> server relay -> servers and back, all on the loopback.
//!
//! The "servers" are echo sockets that answer every datagram with the address and port they saw it from in front of
//! it, so a test sees what the lobby and the match servers would see.

use crate::client::{ClientConfig, ClientRelay};
use crate::frame::{Header, Hello, Kind, Welcome, MAX_INNER};
use crate::server::{is_player_address, ServerConfig, ServerRelay};
use crate::session::Session;
use crate::suite::tests::ToyCipher;
use crate::suite::{Accepted, CipherPair, ClientHandshake, Suite};
use crate::Redundancy;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Echo {
    port: u16,
    received: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
}

impl Echo {
    fn start() -> Echo {
        let sock = UdpSocket::bind("127.0.0.1:0").unwrap();
        sock.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
        let port = sock.local_addr().unwrap().port();
        let received = Arc::new(AtomicU64::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let (r, s) = (received.clone(), stop.clone());
        std::thread::spawn(move || {
            let mut buf = vec![0u8; 65536];
            while !s.load(Ordering::Relaxed) {
                let Ok((n, from)) = sock.recv_from(&mut buf) else { continue };
                r.fetch_add(1, Ordering::Relaxed);
                let IpAddr::V4(ip) = from.ip() else { continue };
                let mut out = ip.octets().to_vec();
                out.extend_from_slice(&from.port().to_le_bytes());
                out.extend_from_slice(&buf[..n]);
                let _ = sock.send_to(&out, from);
            }
        });
        Echo { port, received, stop }
    }
}

impl Drop for Echo {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn server(ports: Vec<u16>) -> ServerConfig {
    let mut c = ServerConfig::new("127.0.0.1:0".parse().unwrap(), ports);
    c.whois = Some("127.0.0.1:0".parse().unwrap());
    c
}

fn client(server: &ServerRelay, ports: &[u16]) -> ClientConfig {
    let mut c = ClientConfig::new(server.local_addr(), ports);
    c.ports = ports.iter().map(|&p| (p, 0)).collect();
    c.handshake_timeout = Duration::from_secs(3);
    c
}

/// A game: one socket for every server, as the real one.
struct Game {
    sock: UdpSocket,
}

impl Game {
    fn new() -> Game {
        let sock = UdpSocket::bind("127.0.0.1:0").unwrap();
        sock.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        Game { sock }
    }

    /// Sends to a local port, returns where the server saw it from and checks the echo.
    fn ask(&self, local_port: u16, payload: &[u8]) -> SocketAddr {
        self.sock.send_to(payload, ("127.0.0.1", local_port)).unwrap();
        let mut buf = vec![0u8; 65536];
        let (n, from) = self.sock.recv_from(&mut buf).expect("no answer through the tunnel");
        assert_eq!(from.port(), local_port, "the answer comes from the port the game sent to");
        assert_eq!(&buf[6..n], payload, "the datagram arrives whole");
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(buf[0], buf[1], buf[2], buf[3])), u16::from_le_bytes([buf[4], buf[5]]))
    }
}

fn pattern(len: usize, seed: u8) -> Vec<u8> {
    (0..len).map(|i| (i as u8).wrapping_mul(31).wrapping_add(seed)).collect()
}

fn whois(addr: SocketAddr, q: &str) -> String {
    let s = UdpSocket::bind("127.0.0.1:0").unwrap();
    s.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
    s.send_to(q.as_bytes(), addr).unwrap();
    let mut buf = [0u8; 65536];
    let (n, _) = s.recv_from(&mut buf).unwrap();
    String::from_utf8_lossy(&buf[..n]).into_owned()
}

fn wait_for(what: &str, mut f: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !f() {
        assert!(Instant::now() < deadline, "timed out waiting for {}", what);
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn many_players_keep_their_own_address_both_ways() {
    let (lobby, matchs) = (Echo::start(), Echo::start());
    let mut cfg = server(vec![lobby.port, matchs.port]);
    cfg.max_per_address = 32; // every test player comes from 127.0.0.1
    let relay = ServerRelay::start(cfg).unwrap();
    let w = relay.whois_addr().unwrap();
    let sizes = [1usize, 14, 100, 548, 981, 1300, 1350, 1400, MAX_INNER, 1472];
    let players = 12;
    let results: Vec<(SocketAddr, SocketAddr, u16, ClientRelay)> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..players)
            .map(|i| {
                let relay = &relay;
                let (lp, mp) = (lobby.port, matchs.port);
                s.spawn(move || {
                    let mut cfg = client(relay, &[lp, mp]);
                    cfg.label = format!("player{}", i);
                    let c = ClientRelay::start(cfg).unwrap();
                    let game = Game::new();
                    let (l, m) = (c.local_port(lp).unwrap(), c.local_port(mp).unwrap());
                    let mut seen_lobby = None;
                    let mut seen_match = None;
                    for round in 0..3 {
                        for &n in &sizes {
                            let a = game.ask(l, &pattern(n, i as u8 ^ round));
                            let b = game.ask(m, &pattern(n, !(i as u8) ^ round));
                            assert_eq!(*seen_lobby.get_or_insert(a), a, "the lobby always sees one address and port");
                            assert_eq!(*seen_match.get_or_insert(b), b);
                        }
                    }
                    let st = c.stats();
                    assert_eq!(st.up_datagrams, (sizes.len() * 6) as u64);
                    assert_eq!(st.down_datagrams, (sizes.len() * 6) as u64);
                    assert_eq!(st.dropped, 0);
                    assert_eq!(st.oversize, 6, "only the 1472-byte datagrams are over the budget");
                    assert_eq!((st.up_largest, st.down_largest), (1472, 1472 + 6), "the echo puts 6 bytes in front");
                    (seen_lobby.unwrap(), seen_match.unwrap(), l, c)
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let mut ips: Vec<IpAddr> = Vec::new();
    for (lobby_saw, match_saw, local_lobby_port, _relay) in &results {
        assert!(is_player_address(lobby_saw.ip()), "{} is a player address", lobby_saw);
        assert_eq!(lobby_saw, match_saw, "lobby and match see the player at the same address and port");
        let answer = whois(w, &format!("WHOIS {}", lobby_saw.ip()));
        assert!(answer.starts_with(&format!("PEER {} 127.0.0.1:", lobby_saw.ip())), "{}", answer);
        assert!(answer.contains(&format!("{}={}", lobby.port, local_lobby_port)), "{}", answer);
        assert!(answer.contains(" player"), "{}", answer);
        ips.push(lobby_saw.ip());
    }
    // Every player its own address, each in a /16 of its own.
    let mut nets: Vec<u8> = ips.iter().map(|ip| if let IpAddr::V4(v) = ip { v.octets()[1] } else { 0 }).collect();
    nets.sort();
    nets.dedup();
    assert_eq!(nets.len(), players);
    let list = whois(w, "LIST");
    assert!(list.starts_with(&format!("COUNT {}", players)), "{}", list);
    assert_eq!(whois(w, "WHOIS 127.0.0.1"), "NONE 127.0.0.1");
    let st = relay.stats();
    assert_eq!(st.sessions, players as u64);
    assert_eq!(st.up_datagrams, (players * sizes.len() * 6) as u64);
    assert_eq!(st.down_datagrams, st.up_datagrams);
    assert_eq!(st.dropped, 0);
    assert_eq!(lobby.received.load(Ordering::Relaxed) + matchs.received.load(Ordering::Relaxed), st.up_datagrams);
}

#[test]
fn a_datagram_over_the_path_budget_still_arrives() {
    let lobby = Echo::start();
    let relay = ServerRelay::start(server(vec![lobby.port])).unwrap();
    let c = ClientRelay::start(client(&relay, &[lobby.port])).unwrap();
    let game = Game::new();
    let big = pattern(9000, 5);
    game.ask(c.local_port(lobby.port).unwrap(), &big);
    assert_eq!(c.stats().oversize, 1);
    assert_eq!(relay.stats().oversize, 1);
}

#[test]
fn a_player_without_the_tunnel_is_seen_as_before() {
    let lobby = Echo::start();
    let relay = ServerRelay::start(server(vec![lobby.port])).unwrap();
    let c = ClientRelay::start(client(&relay, &[lobby.port])).unwrap();
    let (tunnelled, direct) = (Game::new(), Game::new());
    let via = tunnelled.ask(c.local_port(lobby.port).unwrap(), b"through");
    let plain = direct.ask(lobby.port, b"direct");
    assert!(is_player_address(via.ip()));
    assert_eq!(plain, direct.sock.local_addr().unwrap(), "a direct player is seen at its own address and port");
    assert!(!is_player_address(plain.ip()));
}

#[test]
fn only_the_ports_the_relay_carries() {
    let (lobby, other) = (Echo::start(), Echo::start());
    let relay = ServerRelay::start(server(vec![lobby.port])).unwrap();
    // The client asks for both; the server relay carries one.
    let c = ClientRelay::start(client(&relay, &[lobby.port, other.port])).unwrap();
    let game = Game::new();
    game.ask(c.local_port(lobby.port).unwrap(), b"ok");
    game.sock.send_to(b"nope", ("127.0.0.1", c.local_port(other.port).unwrap())).unwrap();
    wait_for("the drop", || relay.stats().dropped == 1);
    assert_eq!(other.received.load(Ordering::Relaxed), 0);
}

#[test]
fn the_tunnel_comes_back_after_the_server_relay_restarts() {
    let lobby = Echo::start();
    let first = ServerRelay::start(server(vec![lobby.port])).unwrap();
    let addr = first.local_addr();
    let c = ClientRelay::start(client(&first, &[lobby.port])).unwrap();
    let game = Game::new();
    let lp = c.local_port(lobby.port).unwrap();
    game.ask(lp, b"before");
    let old = c.session();
    first.stop();
    let mut cfg = server(vec![lobby.port]);
    cfg.listen = addr;
    let _second = ServerRelay::start(cfg).unwrap();
    // The first datagram after the restart gets a REJECT and is lost (the game resends); the relay says HELLO again.
    game.sock.send_to(b"lost", ("127.0.0.1", lp)).unwrap();
    wait_for("a new session", || c.session() != 0 && c.session() != old);
    game.ask(lp, b"after");
    assert_eq!(c.stats().sessions, 2);
}

#[test]
fn a_silent_session_is_closed_and_a_pinging_one_kept() {
    let lobby = Echo::start();
    let mut cfg = server(vec![lobby.port]);
    cfg.idle_timeout = Duration::from_millis(1200);
    let relay = ServerRelay::start(cfg).unwrap();
    let w = relay.whois_addr().unwrap();
    let mut pinging = client(&relay, &[lobby.port]);
    pinging.keepalive = Duration::from_millis(300);
    let mut silent = client(&relay, &[lobby.port]);
    silent.keepalive = Duration::from_secs(60);
    let (p, s) = (ClientRelay::start(pinging).unwrap(), ClientRelay::start(silent).unwrap());
    let (gp, gs) = (Game::new(), Game::new());
    let ap = gp.ask(p.local_port(lobby.port).unwrap(), b"x");
    let as_ = gs.ask(s.local_port(lobby.port).unwrap(), b"x");
    std::thread::sleep(Duration::from_millis(2600));
    assert!(whois(w, &format!("WHOIS {}", ap.ip())).starts_with("PEER"));
    assert_eq!(whois(w, &format!("WHOIS {}", as_.ip())), format!("NONE {}", as_.ip()));
    assert!(p.stats().rtt_us > 0, "pings measure the round trip");
    // The silent one speaks again: one datagram lost, then a new session at a new address.
    gs.sock.send_to(b"lost", ("127.0.0.1", s.local_port(lobby.port).unwrap())).unwrap();
    wait_for("a new session", || s.stats().sessions == 2);
    let again = gs.ask(s.local_port(lobby.port).unwrap(), b"y");
    assert_ne!(again.ip(), as_.ip(), "a freed address is not given again at once");
}

#[test]
fn bye_frees_the_address_and_reuse_waits() {
    let lobby = Echo::start();
    let mut cfg = server(vec![lobby.port]);
    cfg.reuse_after = Duration::from_secs(3600);
    let relay = ServerRelay::start(cfg).unwrap();
    let w = relay.whois_addr().unwrap();
    let c = ClientRelay::start(client(&relay, &[lobby.port])).unwrap();
    let game = Game::new();
    let a = game.ask(c.local_port(lobby.port).unwrap(), b"x");
    c.stop();
    wait_for("the bye", || relay.stats().sessions_open == 0);
    assert!(whois(w, &format!("WHOIS {}", a.ip())).starts_with("NONE"));
    let c2 = ClientRelay::start(client(&relay, &[lobby.port])).unwrap();
    let b = Game::new().ask(c2.local_port(lobby.port).unwrap(), b"x");
    assert_ne!(a.ip(), b.ip());
}

#[test]
fn too_many_sessions_from_one_address() {
    let lobby = Echo::start();
    let mut cfg = server(vec![lobby.port]);
    cfg.max_per_address = 2;
    let relay = ServerRelay::start(cfg).unwrap();
    let _a = ClientRelay::start(client(&relay, &[lobby.port])).unwrap();
    let _b = ClientRelay::start(client(&relay, &[lobby.port])).unwrap();
    let e = ClientRelay::start(client(&relay, &[lobby.port])).err().expect("the third is refused");
    assert_eq!(e.kind(), std::io::ErrorKind::ConnectionRefused);
}

#[test]
fn no_server_relay_fails_fast() {
    let silent = UdpSocket::bind("127.0.0.1:0").unwrap(); // takes the datagrams, never answers
    let mut cfg = ClientConfig::new(silent.local_addr().unwrap(), &[0]);
    cfg.ports = vec![(21031, 0)];
    cfg.handshake_timeout = Duration::from_millis(600);
    let t = Instant::now();
    let e = ClientRelay::start(cfg).err().expect("no tunnel");
    assert_eq!(e.kind(), std::io::ErrorKind::TimedOut);
    assert!(t.elapsed() < Duration::from_secs(2));
}

#[test]
fn duplicated_small_datagrams_arrive_once() {
    let lobby = Echo::start();
    let mut cfg = server(vec![lobby.port]);
    cfg.redundancy = Redundancy { duplicate_up_to: 400 };
    let relay = ServerRelay::start(cfg).unwrap();
    let mut cc = client(&relay, &[lobby.port]);
    cc.redundancy = Redundancy { duplicate_up_to: 400 };
    let c = ClientRelay::start(cc).unwrap();
    let game = Game::new();
    let lp = c.local_port(lobby.port).unwrap();
    for i in 0..50u8 {
        game.ask(lp, &pattern(if i % 2 == 0 { 40 } else { 900 }, i));
    }
    // Nothing more comes: each echo arrived once.
    game.sock.set_read_timeout(Some(Duration::from_millis(300))).unwrap();
    assert!(game.sock.recv_from(&mut [0u8; 2048]).is_err());
    assert_eq!(lobby.received.load(Ordering::Relaxed), 50);
    assert_eq!(relay.stats().dropped, 25, "the second copy of each small datagram");
    assert_eq!(c.stats().dropped, 25);
}

// ---- frames written by hand: what somebody else on the network could send.

fn raw_session(sock: &UdpSocket, server: SocketAddr, suite: &dyn Suite, ports: Vec<(u16, u16)>) -> Session {
    let (hs, pending) = suite.client_start().unwrap();
    let mut hello = Vec::new();
    Hello { nonce: [9; 16], ports, label: "raw".into(), handshake: hs }.encode(&mut hello);
    sock.send_to(&hello, server).unwrap();
    let mut buf = [0u8; 2048];
    let (n, _) = sock.recv_from(&mut buf).unwrap();
    let (h, body) = Header::parse(&buf[..n]).unwrap();
    assert_eq!(h.kind, Kind::Welcome);
    let w = Welcome::decode(h.session, body).unwrap();
    Session::new(w.session, pending.finish(&w.handshake).unwrap())
}

#[test]
fn in_clear_a_session_cannot_be_used_from_another_address() {
    let lobby = Echo::start();
    let relay = ServerRelay::start(server(vec![lobby.port])).unwrap();
    let owner = UdpSocket::bind("127.0.0.1:0").unwrap();
    owner.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    let s = raw_session(&owner, relay.local_addr(), &crate::suite::NullSuite, vec![(lobby.port, 1)]);
    owner.send_to(&s.seal_data(lobby.port, b"mine"), relay.local_addr()).unwrap();
    wait_for("the owner's datagram", || lobby.received.load(Ordering::Relaxed) == 1);
    let other = UdpSocket::bind("127.0.0.1:0").unwrap();
    other.send_to(&s.seal_data(lobby.port, b"injected"), relay.local_addr()).unwrap();
    wait_for("the drop", || relay.stats().dropped == 1);
    // A repeat of a frame already taken is dropped too.
    let f = s.seal_data(lobby.port, b"once");
    owner.send_to(&f, relay.local_addr()).unwrap();
    owner.send_to(&f, relay.local_addr()).unwrap();
    wait_for("the repeat's drop", || relay.stats().dropped == 2);
    assert_eq!(lobby.received.load(Ordering::Relaxed), 2);
}

/// Step 2 stand-in: a suite whose ciphers authenticate.
struct ToySuite;
struct ToyClient;
impl ClientHandshake for ToyClient {
    fn finish(self: Box<Self>, welcome: &[u8]) -> Result<CipherPair, String> {
        if welcome != b"toy-welcome" {
            return Err("not toy".into());
        }
        Ok(CipherPair { send: Box::new(ToyCipher(1)), recv: Box::new(ToyCipher(2)) })
    }
}
impl Suite for ToySuite {
    fn name(&self) -> &'static str {
        "toy"
    }
    fn client_start(&self) -> Result<(Vec<u8>, Box<dyn ClientHandshake>), String> {
        Ok((b"toy-hello".to_vec(), Box::new(ToyClient)))
    }
    fn server_accept(&self, hello: &[u8]) -> Result<Accepted, String> {
        if hello != b"toy-hello" {
            return Err("not toy".into());
        }
        Ok(Accepted {
            welcome: b"toy-welcome".to_vec(),
            ciphers: CipherPair { send: Box::new(ToyCipher(2)), recv: Box::new(ToyCipher(1)) },
            identity: Some("toy-account".into()),
        })
    }
}

#[test]
fn an_authenticating_suite_carries_and_follows_its_player() {
    let lobby = Echo::start();
    let mut cfg = server(vec![lobby.port]);
    cfg.suite = Arc::new(ToySuite);
    let relay = ServerRelay::start(cfg).unwrap();
    let w = relay.whois_addr().unwrap();
    // Through the relays, with the 16-byte tag on every frame.
    let mut cc = client(&relay, &[lobby.port]);
    cc.suite = Arc::new(ToySuite);
    let c = ClientRelay::start(cc).unwrap();
    let seen = Game::new().ask(c.local_port(lobby.port).unwrap(), &pattern(MAX_INNER, 1));
    assert!(whois(w, &format!("WHOIS {}", seen.ip())).contains(" toy-account "));
    // A client in clear is refused by an authenticating relay.
    let mut plain = client(&relay, &[lobby.port]);
    plain.suite = Arc::new(crate::suite::NullSuite);
    assert_eq!(ClientRelay::start(plain).err().unwrap().kind(), std::io::ErrorKind::ConnectionRefused);
    // By hand: a frame that opens moves the session to its new address; a forged one does not.
    let a = UdpSocket::bind("127.0.0.1:0").unwrap();
    a.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    let s = raw_session(&a, relay.local_addr(), &ToySuite, vec![(lobby.port, 1)]);
    let b = UdpSocket::bind("127.0.0.1:0").unwrap();
    b.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    let before = lobby.received.load(Ordering::Relaxed);
    let mut forged = s.seal_data(lobby.port, b"forged");
    let last = forged.len() - 1;
    forged[last] ^= 0xFF;
    b.send_to(&forged, relay.local_addr()).unwrap();
    wait_for("the forged frame's drop", || relay.stats().dropped >= 1);
    b.send_to(&s.seal_data(lobby.port, b"moved"), relay.local_addr()).unwrap();
    let mut buf = [0u8; 2048];
    let (n, _) = b.recv_from(&mut buf).expect("the answer goes to the new address");
    let (h, _) = Header::parse(&buf[..n]).unwrap();
    let plain = s.open(&h, &buf[..n]).expect("sealed for this session");
    assert_eq!(&plain[2 + 6..], b"moved");
    assert_eq!(lobby.received.load(Ordering::Relaxed), before + 1);
}
