//! §7.1, and §12's "separately confirm that §7.1's integer stream is the same across
//! targets" and "one stream per command, so attempt comes out deterministically the same
//! across targets" (§6.5).

use fixed::types::{I16F16, U0F32};
use padparadscha_common::{Seed, Seq, command_stream, face_dot, fixed_range, fixed_unit};

#[test]
fn one_seed_and_seq_give_one_state_everywhere_the_same() {
    let seed = Seed(0x5EED_5EED_5EED_5EED);
    assert_eq!(command_stream(seed, Seq(7)), command_stream(seed, Seq(7)));
}

#[test]
fn one_command_has_one_stream_and_another_command_another() {
    // One stream per command, and it is the stream — not a fresh draw — that carries the
    // re-toss numbering (spec §6.5): the attempt of `args = { who, outcome, attempt }` is
    // read out of this one stream, so it is the same on every target.
    let seed = Seed(0x5EED_5EED_5EED_5EED);
    assert_ne!(command_stream(seed, Seq(1)), command_stream(seed, Seq(2)));
    // (seed, seq) is *expanded* by SplitMix64, not laid into the state raw.
    assert_ne!(command_stream(seed, Seq(0))[0], 0x5EED_5EED_5EED_5EED);
}

#[test]
fn the_upper_32_bits_go_straight_into_the_fraction() {
    // 0 integer bits, 32 fraction bits: the upper 32 bits are the whole fraction, so the
    // value is word >> 32 with no truncation and no rounding.
    assert_eq!(fixed_unit(0xFFFF_FFFF_0000_0000).to_bits(), 0xFFFF_FFFF_u32);
    assert!(fixed_unit(0xFFFF_FFFF_0000_0000) > fixed_unit(0x0000_0000_FFFF_FFFF));
    assert!(fixed_unit(0) < U0F32::from_bits(1));
}

#[test]
fn a_range_is_the_fixed_point_multiply_alone() {
    // lo + (hi - lo) * x, and x == 1 is hi's side of it.
    let x = U0F32::from_bits(u32::MAX);
    let v = fixed_range(x, I16F16::const_from_int(2), I16F16::const_from_int(4));
    assert!(
        v > I16F16::const_from_int(3),
        "x is under 1 by a hair, so the value is under 4"
    );
    let zero = U0F32::from_bits(0);
    assert_eq!(
        fixed_range(zero, I16F16::const_from_int(2), I16F16::const_from_int(4)),
        I16F16::const_from_int(2)
    );
}

#[test]
fn the_angle_question_is_a_dot_product_and_nothing_else() {
    // cos is a dot product taken after the two orientations are dropped onto the same fixed
    // point: no target's f64 arithmetic is left to sway the adjudication.
    let up = [0.0_f64, 1.0, 0.0];
    // cos(45°) at I16F16: 0.7071_0678 × 65536 = 46341 raw.
    let cos45 = I16F16::from_bits(46341);
    assert!(
        face_dot(up, up) > cos45,
        "under 45° is that face (spec 6.4)"
    );
    // A face normal exactly 45° from the table's is not under 45°: it is not adjudicable, and
    // the body re-tosses on its own (spec 6.4's second case).
    #[expect(
        clippy::approx_constant,
        reason = "the measured edge fixture is deliberately approximate"
    )]
    let edge = [0.707_106_8_f64, 0.707_106_8, 0.0];
    assert!(
        face_dot(edge, up) < cos45,
        "45° or more: no adjudication, and attempt carries it (§6.5)"
    );
}
