//! Coin toss as one entry of the engine's capability list (spec §6). The body, the launch its
//! stream decides, the engine's simulate, and the read-out — none of it is per game.

use fixed::types::I16F16;
use padparadscha_common::{Seed, Seq, command_rng, face_dot, fixed_range, fixed_unit};
use rand_core::Rng as _;

use crate::physics::{Coin, Keyframe, Launch, simulate};
use crate::table::Table;

/// What a read-out adjudicates to (§6.4).
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Outcome {
    Heads,
    Tails,
    Edge,
}

impl core::fmt::Display for Outcome {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::Heads => "heads",
            Self::Tails => "tails",
            Self::Edge => "edge",
        })
    }
}

/// The one command's result: the outcome, and which try of that one command's one stream produced
/// it (§6.5).
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct CoinOutcome {
    pub outcome: Outcome,
    /// The re-toss number, starting at 0. Never a new log row (§6.5).
    pub attempt: u8,
}

/// The whole capability: the table it runs on, and the body's own shapes.
pub struct CoinToss {
    pub table: Table,
}

impl CoinToss {
    /// The disc's two face normals when resting flat, in the read-out's world frame.
    pub const FACE_A_WORLD: [f64; 3] = [0.0, 1.0, 0.0];
    pub const FACE_B_WORLD: [f64; 3] = [0.0, -1.0, 0.0];

    /// cos(45°) at I16F16: 0.7071_0678 × 65536.
    const COS45: I16F16 = I16F16::from_bits(46341);

    /// §6.4's angle question, as an `I16F16` dot product alone (no `cos`, no `atan2`).
    pub fn face_cos(&self, face_normal: [f64; 3], table_normal: [f64; 3]) -> I16F16 {
        face_dot(face_normal, table_normal)
    }

    /// Under 45° from the table's normal is that face; 45° or more cannot be adjudicated, so
    /// `None` says "re-toss on its own" (§6.4).
    pub fn adjudicate(&self, cos: I16F16) -> Option<Outcome> {
        if cos > Self::COS45 {
            Some(Outcome::Heads)
        } else if cos < -Self::COS45 {
            Some(Outcome::Tails)
        } else {
            None
        }
    }

    /// One `toss` command: one body spawned on the table with the position, orientation, velocity and
    /// angular velocity the seed decides (§6.2), stepped to rest, read out, and re-tossed out of the
    /// **same** stream until a face is decided or the third try is spent (§6.4, §6.5).
    pub fn toss(&self, seed: Seed, seq: Seq) -> CoinOutcome {
        let mut rng = command_rng(seed, seq);
        // A range is common's fixed-point multiply alone; only its result enters rapier's f64 launch.
        let mut draw = |lo: f64, hi: f64| {
            fixed_range(
                fixed_unit(rng.next_u64()),
                I16F16::from_num(lo),
                I16F16::from_num(hi),
            )
            .to_num::<f64>()
        };
        for attempt in 0..=2 {
            let launch = Launch {
                position: [0.0, self.table.wall_m + Coin::COIN.thickness_m, 0.0],
                // A vertical disc spinning about the table's normal can rest on its rim; the
                // measured (SEED, Seq(0)) fixture spends all three tries there (§6.5).
                axis: [0.0, 0.0, 1.0],
                angle: core::f64::consts::FRAC_PI_2,
                linvel: [0.0, 0.0, 0.0],
                angvel: [0.0, draw(-20.0, 20.0), 0.0],
            };
            let Keyframe { face_normal, .. } =
                simulate(&self.table, crate::session::Body::Coin, launch);
            if let Some(outcome) = self.adjudicate(self.face_cos(face_normal, self.table.normal()))
            {
                return CoinOutcome { outcome, attempt };
            }
            // 45° or more: the next pass draws a fresh launch on this command's one stream (§6.5).
        }
        CoinOutcome {
            outcome: Outcome::Edge,
            attempt: 2,
        }
    }
}
