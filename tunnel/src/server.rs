//! The VPS's side: takes the sessions of the client relays on one UDP port and gives each player its own address
//! towards the servers on the same machine.
//!
//! # Who the servers see
//!
//! The lobby and the match servers know a player by where its datagrams come from: a connection belongs to the
//! address that opened it, the match pass is the address the lobby saw, the session token is checked against the
//! address that signed in (`GRP_TOKEN_BIND`). So each session gets one socket bound to an address of its own,
//! `127.N.0.1` with N from 64 to 254 (Linux routes all of 127/8 to the loopback), and sends to every server port
//! from it, as the game sends from one socket. Lobby and match see the same address for one player; two players
//! never share a /16, so the match pass's "same /16" tolerance never lets one tunnelled player pass for another.
//! 191 players at once; a freed address waits `reuse_after` before the next session takes it, so nothing the
//! servers still hold for the last player carries over.
//!
//! # The whois
//!
//! What the servers cannot see, the real address and the client relay's ports, the lobby asks on a UDP port on
//! 127.0.0.1 (`--whois`): `WHOIS 127.65.0.1` is answered with
//! `PEER 127.65.0.1 <real ip:port> <session> <identity|-> <label|-> <server port>=<local port>,...` or
//! `NONE 127.65.0.1`; `LIST` gives one PEER line per session. Only this machine can ask (the socket is bound to
//! the loopback).

use crate::frame::{self, Header, Hello, Kind, Reject, Welcome, BUF, MAX_INNER};
use crate::session::{split_data, Session};
use crate::suite::{NullSuite, Suite};
use crate::{random_u32_nonzero, Log, Redundancy};
use std::collections::{HashMap, VecDeque};
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// The players' addresses: 127.N.0.1 for N in this range.
pub const FIRST_OCTET: u8 = 64;
pub const LAST_OCTET: u8 = 254;

/// The address of the player at this octet.
pub fn player_address(octet: u8) -> Ipv4Addr {
    Ipv4Addr::new(127, octet, 0, 1)
}

/// Is this one of the players' addresses? (What the lobby's `Tunnel.IsTunnelAddress` checks too.)
pub fn is_player_address(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => {
            let o = v.octets();
            o[0] == 127 && (FIRST_OCTET..=LAST_OCTET).contains(&o[1])
        }
        _ => false,
    }
}

pub struct ServerConfig {
    /// Where the client relays send (0.0.0.0:21035 on the VPS).
    pub listen: SocketAddr,
    /// The servers' address on this machine.
    pub backend: Ipv4Addr,
    /// The server ports a session may reach (lobby 21030-21031, match servers 21032-21034).
    pub ports: Vec<u16>,
    /// The whois socket (127.0.0.1:21039 on the VPS); None: no whois.
    pub whois: Option<SocketAddr>,
    pub suite: Arc<dyn Suite>,
    pub redundancy: Redundancy,
    /// A session that says nothing for this long is closed (the client relay pings every 10 s).
    pub idle_timeout: Duration,
    /// Sessions from one IP address at once (players behind one router).
    pub max_per_address: usize,
    /// How long a freed player address waits before another session takes it.
    pub reuse_after: Duration,
    pub log: Log,
}

impl ServerConfig {
    pub fn new(listen: SocketAddr, ports: Vec<u16>) -> ServerConfig {
        ServerConfig {
            listen,
            backend: Ipv4Addr::LOCALHOST,
            ports,
            whois: None,
            suite: Arc::new(NullSuite),
            redundancy: Redundancy::default(),
            idle_timeout: Duration::from_secs(90),
            max_per_address: 8,
            reuse_after: Duration::from_secs(120),
            log: crate::quiet(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub sessions: u64,
    pub sessions_open: u64,
    /// Datagrams from the players to the servers.
    pub up_datagrams: u64,
    pub up_bytes: u64,
    /// Datagrams from the servers to the players.
    pub down_datagrams: u64,
    pub down_bytes: u64,
    /// Frames and datagrams not carried: unknown session, from another address, did not open, repeats, a port
    /// the relay does not carry.
    pub dropped: u64,
    /// Server datagrams over [`MAX_INNER`].
    pub oversize: u64,
}

#[derive(Default)]
struct Counters {
    sessions: AtomicU64,
    up_datagrams: AtomicU64,
    up_bytes: AtomicU64,
    down_datagrams: AtomicU64,
    down_bytes: AtomicU64,
    dropped: AtomicU64,
    oversize: AtomicU64,
}

struct Peer {
    session: Session,
    octet: u8,
    real: Mutex<SocketAddr>,
    ports: Vec<(u16, u16)>,
    label: String,
    identity: Option<String>,
    backend: UdpSocket,
    welcome: Vec<u8>,
    nonce: [u8; 16],
    last_seen: Mutex<Instant>,
    closed: AtomicBool,
}

impl Peer {
    fn line(&self) -> String {
        let clean = |s: &str| if s.is_empty() { "-".to_string() } else { s.chars().map(|c| if c.is_whitespace() || c.is_control() { '_' } else { c }).collect() };
        format!(
            "PEER {} {} {:08x} {} {} {}",
            player_address(self.octet),
            self.real.lock().unwrap(),
            self.session.id,
            clean(self.identity.as_deref().unwrap_or("")),
            clean(&self.label),
            self.ports.iter().map(|(s, l)| format!("{}={}", s, l)).collect::<Vec<_>>().join(",")
        )
    }
}

#[derive(Default)]
struct Peers {
    by_id: HashMap<u32, Arc<Peer>>,
    by_octet: HashMap<u8, u32>,
    /// Free octets, the longest free first; with when they were freed (None: never used).
    free: VecDeque<(u8, Option<Instant>)>,
}

struct Shared {
    backend: Ipv4Addr,
    ports: Vec<u16>,
    suite: Arc<dyn Suite>,
    redundancy: Redundancy,
    idle_timeout: Duration,
    max_per_address: usize,
    reuse_after: Duration,
    log: Log,
    sock: UdpSocket,
    peers: Mutex<Peers>,
    stop: AtomicBool,
    counters: Counters,
    threads: Mutex<Vec<JoinHandle<()>>>,
}

pub struct ServerRelay {
    shared: Arc<Shared>,
    local: SocketAddr,
    whois: Option<SocketAddr>,
    threads: Vec<JoinHandle<()>>,
}

const TICK: Duration = Duration::from_millis(200);

impl ServerRelay {
    pub fn start(cfg: ServerConfig) -> io::Result<ServerRelay> {
        let sock = UdpSocket::bind(cfg.listen)?;
        sock.set_read_timeout(Some(TICK))?;
        let local = sock.local_addr()?;
        let whois_sock = match cfg.whois {
            Some(a) => {
                let w = UdpSocket::bind(a)?;
                if !w.local_addr()?.ip().is_loopback() {
                    return Err(io::Error::new(io::ErrorKind::InvalidInput, "the whois socket must be on the loopback"));
                }
                w.set_read_timeout(Some(TICK))?;
                Some(w)
            }
            None => None,
        };
        let whois = match &whois_sock {
            Some(w) => Some(w.local_addr()?),
            None => None,
        };
        let peers = Peers { free: (FIRST_OCTET..=LAST_OCTET).map(|o| (o, None)).collect(), ..Default::default() };
        let shared = Arc::new(Shared {
            backend: cfg.backend,
            ports: cfg.ports,
            suite: cfg.suite,
            redundancy: cfg.redundancy,
            idle_timeout: cfg.idle_timeout,
            max_per_address: cfg.max_per_address,
            reuse_after: cfg.reuse_after,
            log: cfg.log,
            sock,
            peers: Mutex::new(peers),
            stop: AtomicBool::new(false),
            counters: Counters::default(),
            threads: Mutex::new(Vec::new()),
        });
        let mut threads = Vec::new();
        {
            let s = shared.clone();
            threads.push(std::thread::Builder::new().name("relay-up".into()).spawn(move || s.tunnel_loop())?);
        }
        {
            let s = shared.clone();
            threads.push(std::thread::Builder::new().name("relay-janitor".into()).spawn(move || s.janitor_loop())?);
        }
        if let Some(w) = whois_sock {
            let s = shared.clone();
            threads.push(std::thread::Builder::new().name("relay-whois".into()).spawn(move || s.whois_loop(w))?);
        }
        (shared.log)(&format!(
            "tunnel relay on {} ({}), servers at {} ports {:?}, whois {}",
            local,
            shared.suite.name(),
            shared.backend,
            shared.ports,
            whois.map(|w| w.to_string()).unwrap_or_else(|| "off".into())
        ));
        Ok(ServerRelay { shared, local, whois, threads })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local
    }

    pub fn whois_addr(&self) -> Option<SocketAddr> {
        self.whois
    }

    pub fn stats(&self) -> Stats {
        let c = &self.shared.counters;
        Stats {
            sessions: c.sessions.load(Ordering::Relaxed),
            sessions_open: self.shared.peers.lock().unwrap().by_id.len() as u64,
            up_datagrams: c.up_datagrams.load(Ordering::Relaxed),
            up_bytes: c.up_bytes.load(Ordering::Relaxed),
            down_datagrams: c.down_datagrams.load(Ordering::Relaxed),
            down_bytes: c.down_bytes.load(Ordering::Relaxed),
            dropped: c.dropped.load(Ordering::Relaxed),
            oversize: c.oversize.load(Ordering::Relaxed),
        }
    }

    /// The whois answer for a player address (what the lobby gets).
    pub fn whois(&self, ip: Ipv4Addr) -> String {
        self.shared.whois_answer(ip)
    }

    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        if self.shared.stop.swap(true, Ordering::SeqCst) {
            return;
        }
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
        let ids: Vec<u32> = self.shared.peers.lock().unwrap().by_id.keys().copied().collect();
        for id in ids {
            self.shared.close(id, "relay stopping");
        }
        let peer_threads: Vec<JoinHandle<()>> = self.shared.threads.lock().unwrap().drain(..).collect();
        for t in peer_threads {
            let _ = t.join();
        }
    }
}

impl Drop for ServerRelay {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl Shared {
    fn drop_one(&self) {
        self.counters.dropped.fetch_add(1, Ordering::Relaxed);
    }

    fn send_frame(&self, frame: &[u8], to: SocketAddr) {
        for _ in 0..self.redundancy.copies(frame.len()) {
            let _ = self.sock.send_to(frame, to);
        }
    }

    fn tunnel_loop(self: &Arc<Self>) {
        let mut buf = vec![0u8; BUF];
        while !self.stop.load(Ordering::Relaxed) {
            let (n, from) = match self.sock.recv_from(&mut buf) {
                Ok(x) => x,
                Err(_) => continue,
            };
            let frame = &buf[..n];
            let Some((header, body)) = Header::parse(frame) else {
                self.drop_one();
                continue;
            };
            match header.kind {
                Kind::Hello => self.on_hello(body, from),
                Kind::Data | Kind::Ping | Kind::Bye => self.on_sealed(header, frame, from),
                _ => self.drop_one(),
            }
        }
    }

    fn on_hello(self: &Arc<Self>, body: &[u8], from: SocketAddr) {
        let Some(hello) = Hello::decode(body) else {
            let _ = self.sock.send_to(&frame::reject(0, Reject::BadHello), from);
            return;
        };
        let mut peers = self.peers.lock().unwrap();
        // A HELLO said again (its WELCOME was lost): the same WELCOME.
        if let Some(p) = peers.by_id.values().find(|p| p.nonce == hello.nonce && *p.real.lock().unwrap() == from) {
            let _ = self.sock.send_to(&p.welcome, from);
            return;
        }
        if peers.by_id.values().filter(|p| p.real.lock().unwrap().ip() == from.ip()).count() >= self.max_per_address {
            drop(peers);
            (self.log)(&format!("hello from {} refused: {} sessions from that address already", from, self.max_per_address));
            let _ = self.sock.send_to(&frame::reject(0, Reject::TooMany), from);
            return;
        }
        let accepted = match self.suite.server_accept(&hello.handshake) {
            Ok(a) => a,
            Err(e) => {
                drop(peers);
                (self.log)(&format!("hello from {} refused: {}", from, e));
                let _ = self.sock.send_to(&frame::reject(0, Reject::BadHello), from);
                return;
            }
        };
        let ready = peers.free.front().map(|(_, freed)| freed.map(|t| t.elapsed() >= self.reuse_after).unwrap_or(true)).unwrap_or(false);
        if !ready {
            drop(peers);
            (self.log)(&format!("hello from {} refused: every player address is taken", from));
            let _ = self.sock.send_to(&frame::reject(0, Reject::Full), from);
            return;
        }
        let (octet, _) = peers.free.pop_front().unwrap();
        let backend = match UdpSocket::bind((player_address(octet), 0)).and_then(|s| s.set_read_timeout(Some(TICK)).map(|_| s)) {
            Ok(s) => s,
            Err(e) => {
                peers.free.push_front((octet, None));
                drop(peers);
                (self.log)(&format!("hello from {} refused: cannot bind {}: {}", from, player_address(octet), e));
                let _ = self.sock.send_to(&frame::reject(0, Reject::Full), from);
                return;
            }
        };
        let mut id = random_u32_nonzero();
        while peers.by_id.contains_key(&id) {
            id = random_u32_nonzero();
        }
        let ports: Vec<(u16, u16)> = hello.ports.iter().copied().filter(|(s, _)| self.ports.contains(s)).collect();
        let mut welcome = Vec::new();
        Welcome { session: id, nonce: hello.nonce, ports: self.ports.clone(), handshake: accepted.welcome }.encode(&mut welcome);
        let peer = Arc::new(Peer {
            session: Session::new(id, accepted.ciphers),
            octet,
            real: Mutex::new(from),
            ports,
            label: hello.label,
            identity: accepted.identity,
            backend,
            welcome,
            nonce: hello.nonce,
            last_seen: Mutex::new(Instant::now()),
            closed: AtomicBool::new(false),
        });
        peers.by_id.insert(id, peer.clone());
        peers.by_octet.insert(octet, id);
        drop(peers);
        self.counters.sessions.fetch_add(1, Ordering::Relaxed);
        (self.log)(&format!("session {:08x} from {} ({}) is {}", id, from, if peer.label.is_empty() { "-" } else { &peer.label }, player_address(octet)));
        let s = self.clone();
        let p = peer.clone();
        match std::thread::Builder::new().name("relay-down".into()).spawn(move || s.backend_loop(p)) {
            Ok(t) => self.threads.lock().unwrap().push(t),
            Err(e) => {
                (self.log)(&format!("session {:08x}: no thread ({}), closed", id, e));
                self.close(id, "no thread");
                return;
            }
        }
        let _ = self.sock.send_to(&peer.welcome, from);
    }

    fn on_sealed(&self, header: Header, frame: &[u8], from: SocketAddr) {
        let peer = self.peers.lock().unwrap().by_id.get(&header.session).cloned();
        let Some(peer) = peer else {
            self.drop_one();
            let _ = self.sock.send_to(&frame::reject(header.session, Reject::UnknownSession), from);
            return;
        };
        let real = *peer.real.lock().unwrap();
        // In clear anybody could write a frame for a session; only an authenticated one may move it.
        if from != real && !peer.session.authenticates() {
            self.drop_one();
            return;
        }
        let Some(plain) = peer.session.open(&header, frame) else {
            self.drop_one();
            return;
        };
        if from != real {
            *peer.real.lock().unwrap() = from;
            (self.log)(&format!("session {:08x} moved from {} to {}", peer.session.id, real, from));
        }
        *peer.last_seen.lock().unwrap() = Instant::now();
        match header.kind {
            Kind::Data => {
                let Some((port, payload)) = split_data(&plain) else {
                    self.drop_one();
                    return;
                };
                if !self.ports.contains(&port) {
                    self.drop_one();
                    return;
                }
                self.counters.up_datagrams.fetch_add(1, Ordering::Relaxed);
                self.counters.up_bytes.fetch_add(payload.len() as u64, Ordering::Relaxed);
                let _ = peer.backend.send_to(payload, (self.backend, port));
            }
            Kind::Ping => {
                let pong = peer.session.seal(Kind::Pong, &plain);
                let _ = self.sock.send_to(&pong, from);
            }
            Kind::Bye => self.close(peer.session.id, "bye"),
            _ => {}
        }
    }

    /// What the servers send to one player, into its session.
    fn backend_loop(&self, peer: Arc<Peer>) {
        let mut buf = vec![0u8; BUF];
        while !peer.closed.load(Ordering::Relaxed) && !self.stop.load(Ordering::Relaxed) {
            let (n, from) = match peer.backend.recv_from(&mut buf) {
                Ok(x) => x,
                Err(_) => continue,
            };
            if from.ip() != IpAddr::V4(self.backend) || !self.ports.contains(&from.port()) {
                self.drop_one();
                continue;
            }
            if n > MAX_INNER {
                self.counters.oversize.fetch_add(1, Ordering::Relaxed);
            }
            self.counters.down_datagrams.fetch_add(1, Ordering::Relaxed);
            self.counters.down_bytes.fetch_add(n as u64, Ordering::Relaxed);
            let frame = peer.session.seal_data(from.port(), &buf[..n]);
            let to = *peer.real.lock().unwrap();
            self.send_frame(&frame, to);
        }
    }

    fn close(&self, id: u32, why: &str) {
        let mut peers = self.peers.lock().unwrap();
        let Some(peer) = peers.by_id.remove(&id) else { return };
        peers.by_octet.remove(&peer.octet);
        peers.free.push_back((peer.octet, Some(Instant::now())));
        drop(peers);
        peer.closed.store(true, Ordering::Relaxed);
        (self.log)(&format!("session {:08x} ({}) closed: {}", id, player_address(peer.octet), why));
    }

    fn janitor_loop(&self) {
        let mut last = Instant::now();
        while !self.stop.load(Ordering::Relaxed) {
            std::thread::sleep(TICK);
            if last.elapsed() < Duration::from_secs(1) {
                continue;
            }
            last = Instant::now();
            let idle: Vec<u32> = self
                .peers
                .lock()
                .unwrap()
                .by_id
                .values()
                .filter(|p| p.last_seen.lock().unwrap().elapsed() >= self.idle_timeout)
                .map(|p| p.session.id)
                .collect();
            for id in idle {
                self.close(id, "silent");
            }
            // Threads of closed sessions end within a tick; forget them.
            let mut threads = self.threads.lock().unwrap();
            let (done, running): (Vec<_>, Vec<_>) = threads.drain(..).partition(|t| t.is_finished());
            *threads = running;
            drop(threads);
            for t in done {
                let _ = t.join();
            }
        }
    }

    fn whois_answer(&self, ip: Ipv4Addr) -> String {
        let o = ip.octets();
        let peers = self.peers.lock().unwrap();
        if o[0] == 127 && o[2] == 0 && o[3] == 1 {
            if let Some(p) = peers.by_octet.get(&o[1]).and_then(|id| peers.by_id.get(id)) {
                return p.line();
            }
        }
        format!("NONE {}", ip)
    }

    fn whois_loop(&self, sock: UdpSocket) {
        let mut buf = [0u8; 512];
        while !self.stop.load(Ordering::Relaxed) {
            let (n, from) = match sock.recv_from(&mut buf) {
                Ok(x) => x,
                Err(_) => continue,
            };
            if !from.ip().is_loopback() {
                continue;
            }
            let req = String::from_utf8_lossy(&buf[..n]);
            let mut words = req.split_whitespace();
            let answer = match (words.next(), words.next()) {
                (Some("WHOIS"), Some(a)) => match a.parse::<Ipv4Addr>() {
                    Ok(ip) => self.whois_answer(ip),
                    Err(_) => format!("NONE {}", a),
                },
                (Some("LIST"), None) => {
                    let peers = self.peers.lock().unwrap();
                    let mut lines: Vec<String> = peers.by_id.values().map(|p| p.line()).collect();
                    lines.sort();
                    format!("COUNT {}\n{}", lines.len(), lines.join("\n"))
                }
                _ => "ERR WHOIS <address> | LIST".to_string(),
            };
            let _ = sock.send_to(answer.trim_end().as_bytes(), from);
        }
    }
}
