//! One side of a session: its id, its ciphers, the counter of what it sends and the window of what it took.

use crate::frame::{Header, Kind, HEADER_LEN};
use crate::replay::Window;
use crate::suite::CipherPair;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

pub(crate) struct Session {
    pub id: u32,
    ciphers: CipherPair,
    sent: AtomicU64,
    window: Mutex<Window>,
}

impl Session {
    pub fn new(id: u32, ciphers: CipherPair) -> Session {
        Session { id, ciphers, sent: AtomicU64::new(0), window: Mutex::new(Window::new()) }
    }

    pub fn authenticates(&self) -> bool {
        self.ciphers.recv.authenticates()
    }

    /// A whole frame of this kind with `plain` sealed in it.
    pub fn seal(&self, kind: Kind, plain: &[u8]) -> Vec<u8> {
        let counter = self.sent.fetch_add(1, Ordering::Relaxed) + 1;
        let header = Header { kind, session: self.id, counter }.bytes();
        let mut out = Vec::with_capacity(HEADER_LEN + plain.len() + self.ciphers.send.overhead());
        out.extend_from_slice(&header);
        self.ciphers.send.seal(&header, counter, plain, &mut out);
        out
    }

    /// A DATA frame: the server port and the game's datagram.
    pub fn seal_data(&self, port: u16, payload: &[u8]) -> Vec<u8> {
        let mut plain = Vec::with_capacity(2 + payload.len());
        plain.extend_from_slice(&port.to_le_bytes());
        plain.extend_from_slice(payload);
        self.seal(Kind::Data, &plain)
    }

    /// The opened body of a frame of this session, once: None when it does not open or its counter was taken.
    /// The window is only touched by frames that opened, so a forged one cannot use up a counter.
    pub fn open(&self, header: &Header, frame: &[u8]) -> Option<Vec<u8>> {
        let mut out = Vec::with_capacity(frame.len());
        self.ciphers.recv.open(&frame[..HEADER_LEN], header.counter, &frame[HEADER_LEN..], &mut out).ok()?;
        if !self.window.lock().unwrap().accept(header.counter) {
            return None;
        }
        Some(out)
    }
}

/// A DATA body: (server port, datagram).
pub(crate) fn split_data(plain: &[u8]) -> Option<(u16, &[u8])> {
    if plain.len() < 2 {
        return None;
    }
    Some((u16::from_le_bytes([plain[0], plain[1]]), &plain[2..]))
}

pub(crate) fn micros_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_micros() as u64).unwrap_or(0)
}
