//! How a session's datagrams are protected: the seam for step 2.
//!
//! A [`Suite`] runs the handshake (the bytes a HELLO and its WELCOME carry) and gives each side a [`CipherPair`].
//! Every frame after the handshake goes through a [`Cipher`]: `seal` with the 14 header bytes as associated data
//! and the frame's counter as nonce, `open` the same way. Step 1 is [`NullSuite`]: empty handshake, ciphers that
//! copy. Step 2 is a Noise suite (`Noise_NK_25519_ChaChaPoly_BLAKE2s` with the server relay's static key pinned in
//! the launcher, the session token inside the first message) whose split gives two ChaCha20-Poly1305 keys: nothing
//! else in the relays changes. A suite whose ciphers authenticate also lets a session follow its player to a new
//! address (a NAT that moves the port); with [`NullSuite`] a session stays with the address that said HELLO.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CipherError;

impl fmt::Display for CipherError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the frame does not open")
    }
}

pub trait Cipher: Send + Sync {
    /// Bytes `seal` adds (the tag).
    fn overhead(&self) -> usize;
    /// True when `open` proves who sealed (an AEAD); false for [`NullCipher`].
    fn authenticates(&self) -> bool;
    /// Appends the sealed `plain` to `out`.
    fn seal(&self, aad: &[u8], counter: u64, plain: &[u8], out: &mut Vec<u8>);
    /// Appends the opened `sealed` to `out`.
    fn open(&self, aad: &[u8], counter: u64, sealed: &[u8], out: &mut Vec<u8>) -> Result<(), CipherError>;
}

/// One cipher per direction.
pub struct CipherPair {
    pub send: Box<dyn Cipher>,
    pub recv: Box<dyn Cipher>,
}

/// The client's side of a handshake between its HELLO and the WELCOME.
pub trait ClientHandshake: Send {
    fn finish(self: Box<Self>, welcome: &[u8]) -> Result<CipherPair, String>;
}

/// What the server relay learns from a HELLO it accepts.
pub struct Accepted {
    pub welcome: Vec<u8>,
    pub ciphers: CipherPair,
    /// Who the handshake proved this is (step 2: the account of the session token); None in step 1.
    pub identity: Option<String>,
}

pub trait Suite: Send + Sync {
    fn name(&self) -> &'static str;
    /// The client's first message (into the HELLO) and its state until the WELCOME.
    fn client_start(&self) -> Result<(Vec<u8>, Box<dyn ClientHandshake>), String>;
    /// The server's answer to a HELLO's handshake bytes.
    fn server_accept(&self, hello: &[u8]) -> Result<Accepted, String>;
}

/// Step 1: carries datagrams as they are.
pub struct NullCipher;

impl Cipher for NullCipher {
    fn overhead(&self) -> usize {
        0
    }
    fn authenticates(&self) -> bool {
        false
    }
    fn seal(&self, _aad: &[u8], _counter: u64, plain: &[u8], out: &mut Vec<u8>) {
        out.extend_from_slice(plain);
    }
    fn open(&self, _aad: &[u8], _counter: u64, sealed: &[u8], out: &mut Vec<u8>) -> Result<(), CipherError> {
        out.extend_from_slice(sealed);
        Ok(())
    }
}

pub struct NullSuite;

struct NullClient;

impl ClientHandshake for NullClient {
    fn finish(self: Box<Self>, welcome: &[u8]) -> Result<CipherPair, String> {
        if !welcome.is_empty() {
            return Err("the server relay answered with a handshake: it runs another suite".into());
        }
        Ok(null_pair())
    }
}

fn null_pair() -> CipherPair {
    CipherPair { send: Box::new(NullCipher), recv: Box::new(NullCipher) }
}

impl Suite for NullSuite {
    fn name(&self) -> &'static str {
        "null"
    }
    fn client_start(&self) -> Result<(Vec<u8>, Box<dyn ClientHandshake>), String> {
        Ok((Vec::new(), Box::new(NullClient)))
    }
    fn server_accept(&self, hello: &[u8]) -> Result<Accepted, String> {
        if !hello.is_empty() {
            return Err("the HELLO carries a handshake: the client runs another suite".into());
        }
        Ok(Accepted { welcome: Vec::new(), ciphers: null_pair(), identity: None })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A stand-in for step 2 in the tests: XOR with a key byte and a 16-byte check, so a frame that was not sealed
    /// with the key does not open and the relays' handling of `overhead` and `authenticates` is exercised.
    pub struct ToyCipher(pub u8);

    impl ToyCipher {
        fn tag(&self, aad: &[u8], counter: u64, body: &[u8]) -> [u8; 16] {
            let mut t = [self.0; 16];
            for (i, b) in aad.iter().chain(counter.to_le_bytes().iter()).chain(body.iter()).enumerate() {
                t[i % 16] = t[i % 16].rotate_left(3) ^ b;
            }
            t
        }
    }

    impl Cipher for ToyCipher {
        fn overhead(&self) -> usize {
            16
        }
        fn authenticates(&self) -> bool {
            true
        }
        fn seal(&self, aad: &[u8], counter: u64, plain: &[u8], out: &mut Vec<u8>) {
            let start = out.len();
            out.extend(plain.iter().map(|b| b ^ self.0));
            let tag = self.tag(aad, counter, &out[start..]);
            out.extend_from_slice(&tag);
        }
        fn open(&self, aad: &[u8], counter: u64, sealed: &[u8], out: &mut Vec<u8>) -> Result<(), CipherError> {
            if sealed.len() < 16 {
                return Err(CipherError);
            }
            let (body, tag) = sealed.split_at(sealed.len() - 16);
            if self.tag(aad, counter, body) != tag {
                return Err(CipherError);
            }
            out.extend(body.iter().map(|b| b ^ self.0));
            Ok(())
        }
    }

    #[test]
    fn null_copies() {
        let mut out = Vec::new();
        NullCipher.seal(b"h", 1, b"abc", &mut out);
        assert_eq!(out, b"abc");
        let mut back = Vec::new();
        NullCipher.open(b"h", 1, &out, &mut back).unwrap();
        assert_eq!(back, b"abc");
    }

    #[test]
    fn toy_detects_tampering() {
        let c = ToyCipher(0x5A);
        let mut out = Vec::new();
        c.seal(b"header", 7, b"payload", &mut out);
        let mut back = Vec::new();
        c.open(b"header", 7, &out, &mut back).unwrap();
        assert_eq!(back, b"payload");
        assert!(c.open(b"header", 8, &out, &mut Vec::new()).is_err());
        out[0] ^= 1;
        assert!(c.open(b"header", 7, &out, &mut Vec::new()).is_err());
    }

    #[test]
    fn null_suite_handshake() {
        let (hello, pending) = NullSuite.client_start().unwrap();
        let acc = NullSuite.server_accept(&hello).unwrap();
        pending.finish(&acc.welcome).unwrap();
        assert!(NullSuite.server_accept(b"noise").is_err());
    }
}
