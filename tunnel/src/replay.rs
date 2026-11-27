//! Which counters a session has already taken: drops replays (step 2) and the second copy of a duplicated
//! datagram (step 3). A sliding window of 1024 counters behind the highest seen, as WireGuard's (RFC 6479); older
//! ones are dropped too, which only happens to a datagram that lost a race by more than 1024 others.

const WORDS: usize = 16;
const SIZE: u64 = (WORDS * 64) as u64;

#[derive(Default)]
pub struct Window {
    top: u64,
    bits: [u64; WORDS],
}

impl Window {
    pub fn new() -> Window {
        Window::default()
    }

    /// True the first time a counter comes (and marks it taken); false for 0, a repeat, or one too old.
    pub fn accept(&mut self, counter: u64) -> bool {
        if counter == 0 {
            return false;
        }
        if counter > self.top {
            let shift = counter - self.top;
            if shift >= SIZE {
                self.bits = [0; WORDS];
            } else {
                for c in self.top + 1..=counter {
                    self.clear(c);
                }
            }
            self.top = counter;
        } else if self.top - counter >= SIZE {
            return false;
        }
        let (w, b) = Self::slot(counter);
        if self.bits[w] & (1 << b) != 0 {
            return false;
        }
        self.bits[w] |= 1 << b;
        true
    }

    fn slot(c: u64) -> (usize, u32) {
        (((c / 64) % WORDS as u64) as usize, (c % 64) as u32)
    }

    fn clear(&mut self, c: u64) {
        let (w, b) = Self::slot(c);
        self.bits[w] &= !(1 << b);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_order_and_repeats() {
        let mut w = Window::new();
        assert!(!w.accept(0));
        for c in 1..5000 {
            assert!(w.accept(c));
            assert!(!w.accept(c));
        }
    }

    #[test]
    fn out_of_order_inside_the_window() {
        let mut w = Window::new();
        assert!(w.accept(100));
        assert!(w.accept(50));
        assert!(!w.accept(50));
        assert!(w.accept(99));
        assert!(w.accept(2000)); // jumps: 100 is now 1900 behind
        assert!(!w.accept(100));
        assert!(w.accept(1500));
        assert!(!w.accept(1500));
        assert!(!w.accept(2000 - 1024));
        assert!(w.accept(2000 - 1023));
    }

    #[test]
    fn a_big_jump_forgets_everything() {
        let mut w = Window::new();
        for c in 1..=64 {
            assert!(w.accept(c));
        }
        assert!(w.accept(1_000_000));
        assert!(w.accept(1_000_000 - 64)); // slot reused, cleared
    }
}
