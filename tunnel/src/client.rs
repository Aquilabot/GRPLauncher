//! The launcher's side: listens on 127.0.0.1 where the game expects its servers and carries every datagram to
//! the server relay in one session.
//!
//! One relay serves one game. The game sends to the lobby and to the match servers from one UDP socket; the
//! relay remembers, per listening port, where the game last sent from and hands it the answers from that port.
//! The session is made before [`ClientRelay::start`] returns, so the launcher knows the tunnel works before it
//! starts the game (and can start it without the tunnel when it does not). If the server relay forgets the
//! session (restarted, or it timed out) the relay says HELLO again on its own; the servers then see the player at
//! a new address, which the lobby treats as a new connection.

use crate::frame::{Header, Hello, Kind, Reject, Welcome, BUF, HEADER_LEN, MAX_INNER, PORT_LEN};
use crate::session::{micros_now, split_data, Session};
use crate::suite::{ClientHandshake, NullSuite, Suite};
use crate::{random_bytes, Log, Redundancy};
use std::io;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub struct ClientConfig {
    /// The server relay (the VPS, UDP 21035).
    pub server: SocketAddr,
    /// Where the game is told its servers are.
    pub listen_ip: Ipv4Addr,
    /// (server port, local port): the relay listens on each local port for the server port. Local port 0 takes a
    /// free one (tests: several relays on one machine; [`ClientRelay::local_port`] says which).
    pub ports: Vec<(u16, u16)>,
    /// Who this is, for the server relay's log.
    pub label: String,
    pub suite: Arc<dyn Suite>,
    pub redundancy: Redundancy,
    /// A PING after this long without sending anything, so the server relay keeps the session.
    pub keepalive: Duration,
    /// How long `start` waits for the WELCOME.
    pub handshake_timeout: Duration,
    pub log: Log,
}

impl ClientConfig {
    /// The relay the launcher runs: the same port numbers on 127.0.0.1 as on the server, in clear (step 1).
    pub fn new(server: SocketAddr, server_ports: &[u16]) -> ClientConfig {
        ClientConfig {
            server,
            listen_ip: Ipv4Addr::LOCALHOST,
            ports: server_ports.iter().map(|&p| (p, p)).collect(),
            label: String::new(),
            suite: Arc::new(NullSuite),
            redundancy: Redundancy::default(),
            keepalive: Duration::from_secs(10),
            handshake_timeout: Duration::from_secs(5),
            log: crate::quiet(),
        }
    }
}

/// What a relay has carried.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub up_datagrams: u64,
    pub up_bytes: u64,
    pub down_datagrams: u64,
    pub down_bytes: u64,
    /// Datagrams that were not carried (no session yet, no game to hand them to, did not open, repeats).
    pub dropped: u64,
    /// Game datagrams over [`MAX_INNER`]: carried, but the IP layer fragments them.
    pub oversize: u64,
    /// Sessions made (more than one: the server relay forgot one and the relay said HELLO again).
    pub sessions: u64,
    /// Round trip of the last PING, microseconds (0: none yet).
    pub rtt_us: u64,
    /// The largest game datagram carried each way (the size budget in practice: [`crate::frame`]).
    pub up_largest: u64,
    pub down_largest: u64,
}

#[derive(Default)]
struct Counters {
    up_datagrams: AtomicU64,
    up_bytes: AtomicU64,
    down_datagrams: AtomicU64,
    down_bytes: AtomicU64,
    dropped: AtomicU64,
    oversize: AtomicU64,
    sessions: AtomicU64,
    rtt_us: AtomicU64,
    up_largest: AtomicU64,
    down_largest: AtomicU64,
}

struct Listener {
    server_port: u16,
    local_port: u16,
    sock: UdpSocket,
    game: Mutex<Option<SocketAddr>>,
}

struct Pending {
    nonce: [u8; 16],
    hello: Vec<u8>,
    handshake: Option<Box<dyn ClientHandshake>>,
    sent: Instant,
}

struct Shared {
    server: SocketAddr,
    label: String,
    suite: Arc<dyn Suite>,
    redundancy: Redundancy,
    keepalive: Duration,
    log: Log,
    tunnel: UdpSocket,
    listeners: Vec<Listener>,
    session: Mutex<Option<Arc<Session>>>,
    changed: Condvar,
    pending: Mutex<Option<Pending>>,
    refused: Mutex<Option<Reject>>,
    last_sent: Mutex<Instant>,
    stop: AtomicBool,
    counters: Counters,
}

pub struct ClientRelay {
    shared: Arc<Shared>,
    threads: Vec<JoinHandle<()>>,
}

const TICK: Duration = Duration::from_millis(100);
const HELLO_EVERY: Duration = Duration::from_millis(250);

impl ClientRelay {
    /// Binds the ports, makes the session and starts carrying. Fails when a port is taken, or when the server
    /// relay does not answer or refuses within `handshake_timeout`.
    pub fn start(cfg: ClientConfig) -> io::Result<ClientRelay> {
        let tunnel = UdpSocket::bind(if cfg.server.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" })?;
        tunnel.set_read_timeout(Some(TICK))?;
        let mut listeners = Vec::new();
        for &(server_port, local_port) in &cfg.ports {
            let sock = UdpSocket::bind((cfg.listen_ip, local_port))?;
            sock.set_read_timeout(Some(TICK))?;
            let local_port = sock.local_addr()?.port();
            listeners.push(Listener { server_port, local_port, sock, game: Mutex::new(None) });
        }
        let shared = Arc::new(Shared {
            server: cfg.server,
            label: cfg.label,
            suite: cfg.suite,
            redundancy: cfg.redundancy,
            keepalive: cfg.keepalive,
            log: cfg.log,
            tunnel,
            listeners,
            session: Mutex::new(None),
            changed: Condvar::new(),
            pending: Mutex::new(None),
            refused: Mutex::new(None),
            last_sent: Mutex::new(Instant::now()),
            stop: AtomicBool::new(false),
            counters: Counters::default(),
        });
        shared.say_hello().map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        let mut threads = Vec::new();
        {
            let s = shared.clone();
            threads.push(std::thread::Builder::new().name("tunnel-down".into()).spawn(move || s.tunnel_loop())?);
        }
        {
            let s = shared.clone();
            threads.push(std::thread::Builder::new().name("tunnel-timer".into()).spawn(move || s.timer_loop())?);
        }
        for i in 0..shared.listeners.len() {
            let s = shared.clone();
            threads.push(std::thread::Builder::new().name("tunnel-up".into()).spawn(move || s.listen_loop(i))?);
        }
        let relay = ClientRelay { shared, threads };
        let deadline = Instant::now() + cfg.handshake_timeout;
        let mut session = relay.shared.session.lock().unwrap();
        while session.is_none() {
            if let Some(why) = *relay.shared.refused.lock().unwrap() {
                drop(session);
                return Err(io::Error::new(io::ErrorKind::ConnectionRefused, format!("the server relay refused the tunnel: {:?}", why)));
            }
            let now = Instant::now();
            if now >= deadline {
                drop(session);
                return Err(io::Error::new(io::ErrorKind::TimedOut, format!("no answer from the server relay at {}", relay.shared.server)));
            }
            session = relay.shared.changed.wait_timeout(session, (deadline - now).min(TICK)).unwrap().0;
        }
        drop(session);
        Ok(relay)
    }

    /// The local port the relay listens on for this server port.
    pub fn local_port(&self, server_port: u16) -> Option<u16> {
        self.shared.listeners.iter().find(|l| l.server_port == server_port).map(|l| l.local_port)
    }

    /// The session id (0 while there is none).
    pub fn session(&self) -> u32 {
        self.shared.session.lock().unwrap().as_ref().map(|s| s.id).unwrap_or(0)
    }

    pub fn stats(&self) -> Stats {
        let c = &self.shared.counters;
        Stats {
            up_datagrams: c.up_datagrams.load(Ordering::Relaxed),
            up_bytes: c.up_bytes.load(Ordering::Relaxed),
            down_datagrams: c.down_datagrams.load(Ordering::Relaxed),
            down_bytes: c.down_bytes.load(Ordering::Relaxed),
            dropped: c.dropped.load(Ordering::Relaxed),
            oversize: c.oversize.load(Ordering::Relaxed),
            sessions: c.sessions.load(Ordering::Relaxed),
            rtt_us: c.rtt_us.load(Ordering::Relaxed),
            up_largest: c.up_largest.load(Ordering::Relaxed),
            down_largest: c.down_largest.load(Ordering::Relaxed),
        }
    }

    /// Says BYE (the server relay frees the player's address at once) and stops.
    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        if self.shared.stop.swap(true, Ordering::SeqCst) {
            return;
        }
        // Threads first: the timer would otherwise say HELLO again once the session is gone.
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
        if let Some(s) = self.shared.session.lock().unwrap().take() {
            let _ = self.shared.tunnel.send_to(&s.seal(Kind::Bye, &[]), self.shared.server);
        }
        let st = self.stats();
        (self.shared.log)(&format!(
            "tunnel stopped: {} datagrams up ({} bytes, largest {}), {} down ({} bytes, largest {}), {} dropped, {} oversize, {} session(s)",
            st.up_datagrams, st.up_bytes, st.up_largest, st.down_datagrams, st.down_bytes, st.down_largest, st.dropped, st.oversize, st.sessions
        ));
    }
}

impl Drop for ClientRelay {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl Shared {
    /// Starts a new handshake and sends its HELLO.
    fn say_hello(&self) -> Result<(), String> {
        let mut nonce = [0u8; 16];
        random_bytes(&mut nonce);
        let (handshake_bytes, handshake) = self.suite.client_start()?;
        let hello = Hello {
            nonce,
            ports: self.listeners.iter().map(|l| (l.server_port, l.local_port)).collect(),
            label: self.label.clone(),
            handshake: handshake_bytes,
        };
        let mut bytes = Vec::new();
        hello.encode(&mut bytes);
        let _ = self.tunnel.send_to(&bytes, self.server);
        *self.pending.lock().unwrap() = Some(Pending { nonce, hello: bytes, handshake: Some(handshake), sent: Instant::now() });
        Ok(())
    }

    fn send_frame(&self, frame: &[u8]) {
        for _ in 0..self.redundancy.copies(frame.len()) {
            let _ = self.tunnel.send_to(frame, self.server);
        }
        *self.last_sent.lock().unwrap() = Instant::now();
    }

    fn listen_loop(&self, i: usize) {
        let l = &self.listeners[i];
        let mut buf = vec![0u8; BUF];
        while !self.stop.load(Ordering::Relaxed) {
            let (n, from) = match l.sock.recv_from(&mut buf) {
                Ok(x) => x,
                Err(_) => continue, // timeout (or a Windows ICMP report after the game closed its port)
            };
            if !from.ip().is_loopback() {
                self.counters.dropped.fetch_add(1, Ordering::Relaxed);
                continue;
            }
            *l.game.lock().unwrap() = Some(from);
            let session = self.session.lock().unwrap().clone();
            let Some(session) = session else {
                self.counters.dropped.fetch_add(1, Ordering::Relaxed);
                continue;
            };
            if n > MAX_INNER {
                self.counters.oversize.fetch_add(1, Ordering::Relaxed);
            }
            self.counters.up_datagrams.fetch_add(1, Ordering::Relaxed);
            self.counters.up_bytes.fetch_add(n as u64, Ordering::Relaxed);
            self.counters.up_largest.fetch_max(n as u64, Ordering::Relaxed);
            self.send_frame(&session.seal_data(l.server_port, &buf[..n]));
        }
    }

    fn tunnel_loop(&self) {
        let mut buf = vec![0u8; BUF];
        while !self.stop.load(Ordering::Relaxed) {
            let (n, from) = match self.tunnel.recv_from(&mut buf) {
                Ok(x) => x,
                Err(_) => continue,
            };
            if from != self.server {
                continue;
            }
            let frame = &buf[..n];
            let Some((header, body)) = Header::parse(frame) else { continue };
            match header.kind {
                Kind::Welcome => self.on_welcome(header, body),
                Kind::Data | Kind::Pong | Kind::Bye => {
                    let session = self.session.lock().unwrap().clone();
                    let Some(session) = session.filter(|s| s.id == header.session) else {
                        self.counters.dropped.fetch_add(1, Ordering::Relaxed);
                        continue;
                    };
                    let Some(plain) = session.open(&header, frame) else {
                        self.counters.dropped.fetch_add(1, Ordering::Relaxed);
                        continue;
                    };
                    match header.kind {
                        Kind::Data => self.deliver(&plain),
                        Kind::Pong if plain.len() == 8 => {
                            let sent = u64::from_le_bytes(plain[..8].try_into().unwrap());
                            self.counters.rtt_us.store(micros_now().saturating_sub(sent), Ordering::Relaxed);
                        }
                        Kind::Bye => self.lost(header.session, "the server relay closed the session"),
                        _ => {}
                    }
                }
                Kind::Reject => {
                    let why = body.first().and_then(|&b| Reject::from(b));
                    let current = self.session.lock().unwrap().as_ref().map(|s| s.id);
                    if why == Some(Reject::UnknownSession) && current == Some(header.session) {
                        self.lost(header.session, "the server relay does not know the session");
                    } else if current.is_none() && self.pending.lock().unwrap().is_some() {
                        if let Some(why) = why.filter(|w| *w != Reject::UnknownSession) {
                            (self.log)(&format!("tunnel refused by the server relay: {:?}", why));
                            *self.refused.lock().unwrap() = Some(why);
                            self.changed.notify_all();
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn on_welcome(&self, header: Header, body: &[u8]) {
        let Some(w) = Welcome::decode(header.session, body) else { return };
        let mut pending = self.pending.lock().unwrap();
        let Some(p) = pending.as_mut().filter(|p| p.nonce == w.nonce) else { return };
        let Some(handshake) = p.handshake.take() else { return };
        *pending = None;
        drop(pending);
        match handshake.finish(&w.handshake) {
            Ok(ciphers) => {
                let missing: Vec<u16> = self.listeners.iter().map(|l| l.server_port).filter(|p| !w.ports.contains(p)).collect();
                *self.session.lock().unwrap() = Some(Arc::new(Session::new(w.session, ciphers)));
                self.counters.sessions.fetch_add(1, Ordering::Relaxed);
                (self.log)(&format!(
                    "tunnel session {:08x} with {} ({}){}",
                    w.session,
                    self.server,
                    self.suite.name(),
                    if missing.is_empty() { String::new() } else { format!("; the server relay does not carry port(s) {:?}", missing) }
                ));
                self.changed.notify_all();
            }
            Err(e) => {
                (self.log)(&format!("tunnel handshake failed: {}", e));
                *self.refused.lock().unwrap() = Some(Reject::BadHello);
                self.changed.notify_all();
            }
        }
    }

    /// The session is gone on the server's side: drop it and say HELLO again.
    fn lost(&self, id: u32, why: &str) {
        let mut s = self.session.lock().unwrap();
        if s.as_ref().map(|s| s.id) != Some(id) {
            return;
        }
        *s = None;
        drop(s);
        (self.log)(&format!("tunnel session {:08x} lost ({}): saying hello again", id, why));
        if let Err(e) = self.say_hello() {
            (self.log)(&format!("tunnel hello failed: {}", e));
        }
    }

    fn deliver(&self, plain: &[u8]) {
        let Some((port, payload)) = split_data(plain) else { return };
        let Some(l) = self.listeners.iter().find(|l| l.server_port == port) else {
            self.counters.dropped.fetch_add(1, Ordering::Relaxed);
            return;
        };
        let game = *l.game.lock().unwrap();
        match game {
            Some(game) => {
                self.counters.down_datagrams.fetch_add(1, Ordering::Relaxed);
                self.counters.down_bytes.fetch_add(payload.len() as u64, Ordering::Relaxed);
                self.counters.down_largest.fetch_max(payload.len() as u64, Ordering::Relaxed);
                let _ = l.sock.send_to(payload, game);
            }
            None => {
                self.counters.dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    fn timer_loop(&self) {
        while !self.stop.load(Ordering::Relaxed) {
            std::thread::sleep(TICK);
            if self.stop.load(Ordering::Relaxed) {
                break;
            }
            let session = self.session.lock().unwrap().clone();
            match session {
                None => {
                    let mut pending = self.pending.lock().unwrap();
                    match pending.as_mut() {
                        Some(p) if p.sent.elapsed() >= HELLO_EVERY => {
                            let _ = self.tunnel.send_to(&p.hello, self.server);
                            p.sent = Instant::now();
                        }
                        Some(_) => {}
                        None => {
                            drop(pending);
                            let _ = self.say_hello();
                        }
                    }
                }
                Some(s) => {
                    if self.last_sent.lock().unwrap().elapsed() >= self.keepalive {
                        self.send_frame(&s.seal(Kind::Ping, &micros_now().to_le_bytes()));
                    }
                }
            }
        }
    }
}

/// Bytes the tunnel adds to each game datagram with this cipher overhead (step 1: 0, step 2: 16).
pub fn overhead(cipher_overhead: usize) -> usize {
    HEADER_LEN + PORT_LEN + cipher_overhead
}
