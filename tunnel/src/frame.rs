//! The tunnel's datagrams.
//!
//! ```text
//! 0   u8   version, 0xA1 (protocol 1)
//! 1   u8   kind (Kind)
//! 2   u32  session id, little endian (0 in a HELLO)
//! 6   u64  counter, little endian: per session and direction, from 1; the nonce of the cipher and the
//!          replay window's index (0 in HELLO, WELCOME and REJECT)
//! 14  ...  body
//! ```
//!
//! HELLO, WELCOME and REJECT go in clear (their handshake bytes are the suite's business). DATA, PING, PONG and BYE
//! carry a body sealed by the session's cipher with the 14 header bytes as associated data. A DATA body opens to
//! `[u16 server port][the game's datagram]`.
//!
//! # Size budget
//!
//! The game's datagram is not wrapped in another IP/UDP header: the tunnel datagram replaces the game's, so what it
//! adds is `HEADER_LEN + 2` (16 bytes) now, plus the cipher's tag (16 for ChaCha20-Poly1305) in step 2: 32 bytes.
//!
//! The largest datagram of the game's traffic is 981 bytes: the lobby cuts RMC messages in fragments of 963 bytes
//! (`RMC.MaxRmcPayloadSize`) and adds the PRUDP header and checksum; Phoenix's servers sent the same 981 at most in
//! every capture of 16/17-nov (lobby and match, both directions). 981 + 32 = 1013 bytes, under every path:
//! 1472 (Ethernet, IPv4), 1464 (PPPoE), 1452 (IPv6), 1232 (the IPv6 minimum MTU of 1280). Step 3's redundancy
//! duplicates whole datagrams and adds nothing to each. [`MAX_INNER`] is the largest game datagram that still fits
//! a 1472-byte UDP payload with step 2's overhead; a bigger one is still carried (the IP layer fragments it) and
//! counted as oversize.

pub const VERSION: u8 = 0xA1;
pub const HEADER_LEN: usize = 14;
/// The server port in front of a DATA body.
pub const PORT_LEN: usize = 2;
/// The largest UDP payload that crosses an Ethernet path over IPv4 unfragmented.
pub const PATH_UDP_PAYLOAD: usize = 1472;
/// Step 2's tag (ChaCha20-Poly1305).
pub const AEAD_TAG: usize = 16;
/// The largest game datagram that fits [`PATH_UDP_PAYLOAD`] with step 2's overhead: 1440 bytes.
pub const MAX_INNER: usize = PATH_UDP_PAYLOAD - HEADER_LEN - PORT_LEN - AEAD_TAG;
/// The largest datagram the game sends (measured, see above).
pub const GAME_MAX_DATAGRAM: usize = 981;
/// Buffers: the largest UDP payload there is.
pub const BUF: usize = 65536;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Kind {
    Hello = 1,
    Welcome = 2,
    Data = 3,
    Ping = 4,
    Pong = 5,
    Bye = 6,
    Reject = 7,
}

impl Kind {
    fn from(b: u8) -> Option<Kind> {
        Some(match b {
            1 => Kind::Hello,
            2 => Kind::Welcome,
            3 => Kind::Data,
            4 => Kind::Ping,
            5 => Kind::Pong,
            6 => Kind::Bye,
            7 => Kind::Reject,
            _ => return None,
        })
    }

    /// Kinds whose body goes through the session's cipher.
    pub fn sealed(self) -> bool {
        matches!(self, Kind::Data | Kind::Ping | Kind::Pong | Kind::Bye)
    }
}

/// Why a server relay turned a datagram away (the body of a REJECT).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Reject {
    /// No such session (the server relay restarted, or the session timed out): say HELLO again.
    UnknownSession = 1,
    /// Every player address is taken.
    Full = 2,
    /// The HELLO did not parse or the suite refused it.
    BadHello = 3,
    /// Too many sessions from this address.
    TooMany = 4,
}

impl Reject {
    pub fn from(b: u8) -> Option<Reject> {
        Some(match b {
            1 => Reject::UnknownSession,
            2 => Reject::Full,
            3 => Reject::BadHello,
            4 => Reject::TooMany,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    pub kind: Kind,
    pub session: u32,
    pub counter: u64,
}

impl Header {
    pub fn bytes(&self) -> [u8; HEADER_LEN] {
        let mut h = [0u8; HEADER_LEN];
        h[0] = VERSION;
        h[1] = self.kind as u8;
        h[2..6].copy_from_slice(&self.session.to_le_bytes());
        h[6..14].copy_from_slice(&self.counter.to_le_bytes());
        h
    }

    /// The header and the body after it; None for anything that is not a frame of this protocol.
    pub fn parse(buf: &[u8]) -> Option<(Header, &[u8])> {
        if buf.len() < HEADER_LEN || buf[0] != VERSION {
            return None;
        }
        let kind = Kind::from(buf[1])?;
        let session = u32::from_le_bytes(buf[2..6].try_into().ok()?);
        let counter = u64::from_le_bytes(buf[6..14].try_into().ok()?);
        Some((Header { kind, session, counter }, &buf[HEADER_LEN..]))
    }
}

/// A client's HELLO body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hello {
    /// Ties a WELCOME (and a repeated HELLO) to this attempt.
    pub nonce: [u8; 16],
    /// (server port, local port) for each port the client relay listens on: the lobby gives a tunnelled player
    /// 127.0.0.1 and the local port instead of the server's address and port.
    pub ports: Vec<(u16, u16)>,
    /// Who this is, for the server relay's log (the account name; empty when not known).
    pub label: String,
    /// The suite's first handshake message (empty in step 1).
    pub handshake: Vec<u8>,
}

const HELLO_VERSION: u8 = 1;

impl Hello {
    pub fn encode(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&Header { kind: Kind::Hello, session: 0, counter: 0 }.bytes());
        out.push(HELLO_VERSION);
        out.extend_from_slice(&self.nonce);
        out.push(self.ports.len().min(255) as u8);
        for (s, l) in self.ports.iter().take(255) {
            out.extend_from_slice(&s.to_le_bytes());
            out.extend_from_slice(&l.to_le_bytes());
        }
        let label = &self.label.as_bytes()[..self.label.len().min(64)];
        out.push(label.len() as u8);
        out.extend_from_slice(label);
        out.extend_from_slice(&(self.handshake.len() as u16).to_le_bytes());
        out.extend_from_slice(&self.handshake);
    }

    pub fn decode(body: &[u8]) -> Option<Hello> {
        let mut r = Reader(body);
        if r.u8()? != HELLO_VERSION {
            return None;
        }
        let nonce: [u8; 16] = r.take(16)?.try_into().ok()?;
        let n = r.u8()? as usize;
        let mut ports = Vec::with_capacity(n);
        for _ in 0..n {
            ports.push((r.u16()?, r.u16()?));
        }
        let ll = r.u8()? as usize;
        let label = String::from_utf8_lossy(r.take(ll)?).into_owned();
        let hl = r.u16()? as usize;
        let handshake = r.take(hl)?.to_vec();
        Some(Hello { nonce, ports, label, handshake })
    }
}

/// The server relay's answer to a HELLO.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Welcome {
    pub session: u32,
    pub nonce: [u8; 16],
    /// The server ports this relay carries to.
    pub ports: Vec<u16>,
    /// The suite's answer (empty in step 1).
    pub handshake: Vec<u8>,
}

impl Welcome {
    pub fn encode(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&Header { kind: Kind::Welcome, session: self.session, counter: 0 }.bytes());
        out.extend_from_slice(&self.nonce);
        out.push(self.ports.len().min(255) as u8);
        for p in self.ports.iter().take(255) {
            out.extend_from_slice(&p.to_le_bytes());
        }
        out.extend_from_slice(&(self.handshake.len() as u16).to_le_bytes());
        out.extend_from_slice(&self.handshake);
    }

    pub fn decode(session: u32, body: &[u8]) -> Option<Welcome> {
        let mut r = Reader(body);
        let nonce: [u8; 16] = r.take(16)?.try_into().ok()?;
        let n = r.u8()? as usize;
        let mut ports = Vec::with_capacity(n);
        for _ in 0..n {
            ports.push(r.u16()?);
        }
        let hl = r.u16()? as usize;
        let handshake = r.take(hl)?.to_vec();
        Some(Welcome { session, nonce, ports, handshake })
    }
}

pub fn reject(session: u32, why: Reject) -> Vec<u8> {
    let mut v = Header { kind: Kind::Reject, session, counter: 0 }.bytes().to_vec();
    v.push(why as u8);
    v
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.0.len() < n {
            return None;
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Some(a)
    }
    fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|b| b[0])
    }
    fn u16(&mut self) -> Option<u16> {
        self.take(2).map(|b| u16::from_le_bytes([b[0], b[1]]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_round_trip() {
        let h = Header { kind: Kind::Data, session: 0xDEADBEEF, counter: 0x0102030405060708 };
        let mut v = h.bytes().to_vec();
        v.extend_from_slice(b"body");
        let (back, body) = Header::parse(&v).unwrap();
        assert_eq!(back, h);
        assert_eq!(body, b"body");
    }

    #[test]
    fn not_a_frame() {
        assert!(Header::parse(&[0xA1, 3, 0, 0]).is_none()); // short
        let mut v = Header { kind: Kind::Data, session: 1, counter: 1 }.bytes().to_vec();
        v[0] = 0x00; // a PRUDP datagram or anything else
        assert!(Header::parse(&v).is_none());
        v[0] = VERSION;
        v[1] = 99;
        assert!(Header::parse(&v).is_none());
    }

    #[test]
    fn hello_round_trip() {
        let h = Hello { nonce: [7; 16], ports: vec![(21031, 31031), (21032, 31032)], label: "wv".into(), handshake: vec![1, 2, 3] };
        let mut v = Vec::new();
        h.encode(&mut v);
        let (hdr, body) = Header::parse(&v).unwrap();
        assert_eq!(hdr.kind, Kind::Hello);
        assert_eq!(Hello::decode(body).unwrap(), h);
        // Cut anywhere: no panic, no hello.
        for cut in 0..body.len() {
            assert!(Hello::decode(&body[..cut]).is_none());
        }
    }

    #[test]
    fn welcome_round_trip() {
        let w = Welcome { session: 9, nonce: [3; 16], ports: vec![21030, 21031], handshake: vec![] };
        let mut v = Vec::new();
        w.encode(&mut v);
        let (hdr, body) = Header::parse(&v).unwrap();
        assert_eq!(hdr.kind, Kind::Welcome);
        assert_eq!(Welcome::decode(hdr.session, body).unwrap(), w);
    }

    #[test]
    fn budget() {
        assert_eq!(MAX_INNER, 1440);
        // The game's largest datagram fits even the IPv6 minimum path with step 2's overhead.
        assert!(GAME_MAX_DATAGRAM + HEADER_LEN + PORT_LEN + AEAD_TAG <= 1280 - 40 - 8);
    }
}
