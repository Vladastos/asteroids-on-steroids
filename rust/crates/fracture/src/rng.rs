//! Tiny deterministic RNG (xorshift64*), so the crate stays dependency-free and
//! fractures are reproducible. Replaces `System.Random` in the fragment builder.
//! Note: the number stream does NOT match C#'s `System.Random` — determinism is
//! per-Rust-run, which is what replays/tests need.

#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x9E3779B97F4A7C15 } else { seed },
        }
    }

    #[inline]
    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    /// Uniform in [0, 1).
    #[inline]
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Uniform integer in `[0, n)`. Port of `System.Random.Next(n)`'s contract
    /// (not its bit stream — see the module doc).
    #[inline]
    pub fn next_range(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        ((self.next_u32() as u64 * n as u64) >> 32) as usize
    }

    /// True/false with even odds. Port of `rng.Next(2) == 0`.
    #[inline]
    pub fn next_bool(&mut self) -> bool {
        self.next_u32() & 1 == 0
    }
}
