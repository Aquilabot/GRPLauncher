//! The game's UDP through a tunnel (issue #34, step 1).
//!
//! The game only knows how to talk PRUDP to an address and a port. With the tunnel on, the launcher tells it the
//! server is at 127.0.0.1: a client relay ([`client::ClientRelay`]) listens there on the lobby and match ports and
//! carries every datagram to the server relay ([`server::ServerRelay`]) on the VPS, which hands it to the real
//! servers on that machine. Each player gets an address of its own there (127.N.0.1, one /16 per player), so the
//! servers keep telling players apart by where their packets come from, and the server relay answers the lobby
//! which real address and which local ports stand behind each one (its "whois", [`server::whois`]).
//!
//! Step 1 carries the datagrams in clear ([`suite::NullSuite`]). Everything after the handshake already goes
//! through [`suite::Cipher`] (seal/open with the frame header as associated data and a per-direction counter as
//! nonce), and the handshake through [`suite::Suite`], so step 2 (Noise with ChaCha20-Poly1305) is a new suite and
//! nothing else. The counter also feeds a replay window ([`replay::Window`]) that drops duplicates, which is what
//! step 3 needs to send small datagrams twice ([`Redundancy`]).
//!
//! Frame layout and the size budget: [`frame`].

pub mod client;
pub mod frame;
pub mod replay;
pub mod server;
mod session;
pub mod suite;

#[cfg(test)]
mod e2e;

use std::sync::Arc;

/// Where a relay writes what it does (the launcher's log, the server relay's stdout).
pub type Log = Arc<dyn Fn(&str) + Send + Sync>;

/// A log that writes nothing.
pub fn quiet() -> Log {
    Arc::new(|_| {})
}

/// Step 3, first part: a data frame whose sealed size is at most `duplicate_up_to` bytes goes out twice; the
/// receiver's replay window keeps the first copy that arrives. 0 (the default) sends everything once.
#[derive(Clone, Copy, Debug, Default)]
pub struct Redundancy {
    pub duplicate_up_to: usize,
}

impl Redundancy {
    pub fn copies(&self, sealed_len: usize) -> usize {
        if self.duplicate_up_to > 0 && sealed_len <= self.duplicate_up_to {
            2
        } else {
            1
        }
    }
}

/// Random bytes from the standard library alone: SipHash keyed by the process's random seed over a counter and the
/// clock. Good enough for session ids and handshake nonces in step 1, where nothing is secret; step 2's keys come
/// from its own suite.
pub fn random_bytes(out: &mut [u8]) {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    static STATE: std::sync::OnceLock<RandomState> = std::sync::OnceLock::new();
    let state = STATE.get_or_init(RandomState::new);
    for chunk in out.chunks_mut(8) {
        let mut h = state.build_hasher();
        h.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
        let v = h.finish().to_le_bytes();
        chunk.copy_from_slice(&v[..chunk.len()]);
    }
}

pub fn random_u32_nonzero() -> u32 {
    loop {
        let mut b = [0u8; 4];
        random_bytes(&mut b);
        let v = u32::from_le_bytes(b);
        if v != 0 {
            return v;
        }
    }
}

/// "21030-21034" or "21030,21031,21032": the ports, in order, without repeats.
pub fn parse_ports(s: &str) -> Result<Vec<u16>, String> {
    let mut out = Vec::new();
    for part in s.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (a, b) = match part.split_once('-') {
            Some((a, b)) => (a.trim(), b.trim()),
            None => (part, part),
        };
        let a: u16 = a.parse().map_err(|_| format!("bad port {:?}", part))?;
        let b: u16 = b.parse().map_err(|_| format!("bad port {:?}", part))?;
        if a == 0 || b < a || b - a > 64 {
            return Err(format!("bad port range {:?}", part));
        }
        for p in a..=b {
            if !out.contains(&p) {
                out.push(p);
            }
        }
    }
    if out.is_empty() {
        return Err("no ports".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ports_parse() {
        assert_eq!(parse_ports("21030-21034").unwrap(), vec![21030, 21031, 21032, 21033, 21034]);
        assert_eq!(parse_ports("21031, 21032,21031").unwrap(), vec![21031, 21032]);
        assert!(parse_ports("").is_err());
        assert!(parse_ports("21034-21030").is_err());
        assert!(parse_ports("0").is_err());
    }

    #[test]
    fn redundancy_only_for_small_frames() {
        let r = Redundancy { duplicate_up_to: 200 };
        assert_eq!(r.copies(120), 2);
        assert_eq!(r.copies(201), 1);
        assert_eq!(Redundancy::default().copies(10), 1);
    }

    #[test]
    fn random_differs() {
        let mut a = [0u8; 16];
        let mut b = [0u8; 16];
        random_bytes(&mut a);
        random_bytes(&mut b);
        assert_ne!(a, b);
    }
}
