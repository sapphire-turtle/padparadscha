//! The session's one seed, the command's one seq, and the one stream of unsigned integers
//! pulled from the two of them (spec §7.1). No floating-point RNG is in here: f64 arithmetic
//! cannot be let into the randomness.

use core::iter::zip;
use fixed::traits::LossyFrom;
use fixed::types::{I16F16, I16F48, U0F32};
use rand_core::{Rng as _, SeedableRng};
use rand_xoshiro::{SplitMix64, Xoshiro256PlusPlus};

/// The session's one seed: made once when the session is created, and handed to clients at join
/// (spec §7.3). One per session — a re-toss never makes a new one (§6.5).
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct Seed(pub u64);

/// A log row's place in the one append-only sequence (spec §7.3). A command and the
/// `Event` it produces share exactly one of these (§5.1).
#[derive(
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    Debug,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct Seq(pub u64);

/// The one stream of one command: `Xoshiro256PlusPlus`, whose state is u64 × 4, seeded by
/// SplitMix64's expansion of `(seed, seq)` — and nothing else. Task 7 draws what it needs
/// (initial position, orientation, velocity, angular velocity) straight out of this one generator, so
/// the re-toss number is read from the same stream rather than from a fresh draw (§6.5).
pub fn command_rng(seed: Seed, seq: Seq) -> Xoshiro256PlusPlus {
    // The four words of the state, each its own SplitMix64 draw, in order, on every target.
    let mut expander = SplitMix64::seed_from_u64(seed.0 ^ seq.0);
    let mut bytes = [0_u8; 32];
    for word in bytes.as_chunks_mut::<8>().0 {
        word.copy_from_slice(&expander.next_u64().to_le_bytes());
    }
    Xoshiro256PlusPlus::from_seed(bytes)
}

/// The command's first four draws, as unsigned integers only (`Rng::next_u64`). One command has
/// one stream and another command another, and it is out of that one stream that `attempt` is read.
pub fn command_stream(seed: Seed, seq: Seq) -> [u64; 4] {
    let mut rng = command_rng(seed, seq);
    let mut words = [0_u64; 4];
    for word in words.iter_mut() {
        *word = rng.next_u64();
    }
    words
}

/// The upper 32 bits of a word go straight into the fraction of `U0F32` (0 integer bits, 32
/// fraction bits). What comes out is the `[0, 1)` value as-is: no truncation, no rounding.
pub fn fixed_unit(word: u64) -> U0F32 {
    U0F32::from_bits((word >> 32) as u32)
}

/// `[lo, hi)` decided by the fixed-point multiply alone.
pub fn fixed_range(x: U0F32, lo: I16F16, hi: I16F16) -> I16F16 {
    let x = I16F16::lossy_from(x);
    lo + (hi - lo) * x
}

/// §6.4's angle question, as an `I16F16` dot product alone: the two orientations are dropped
/// onto the same fixed point (`LossyFrom`), and no `cos`, no `atan2`, and no target's own
/// `f64` arithmetic is left in. The f64 arrives through `I16F48`: a direct f64 → F16
/// quantization rounds the edge case up to 46341 and flips its comparison; through F48,
/// the lossy F16 read-out lands at 46340, below the 45° boundary.
pub fn face_dot(a: [f64; 3], b: [f64; 3]) -> I16F16 {
    let drop = |v: f64| I16F16::lossy_from(I16F48::from_num(v));
    let mut dot = I16F16::from_bits(0);
    for (x, y) in zip(a, b) {
        dot += drop(x) * drop(y);
    }
    dot
}
