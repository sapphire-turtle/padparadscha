//! The session (spec §5.1): its id, the one seed, and the bodies placed on the table. `Session`
//! is this crate's own session `Resource`: the queue and the log are common's own types (Task 5),
//! so this crate names no resource of its own for them. Common's `Log` has bevy's marker trait.

use padparadscha_common::Seed;

/// What sits on a table. Body shapes are not rebuilt per game (§5.2): the engine carries
/// `simulate` alone and a game names the body it places. Coin toss and the dice that come
/// later are the same "body bouncing on a table"; dice are not implemented this milestone (§1).
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Body {
    /// The disc with thickness of §6.1: two faces and an edge.
    Coin,
}

/// One game: the id, the one seed made when the session was created, and its bodies.
#[derive(bevy::prelude::Resource)]
pub struct Session {
    pub id: String,
    pub seed: Seed,
    pub bodies: Vec<Body>,
}

impl Session {
    pub fn new(id: impl Into<String>, seed: Seed) -> Self {
        Self {
            id: id.into(),
            seed,
            bodies: vec![Body::Coin],
        }
    }
}
