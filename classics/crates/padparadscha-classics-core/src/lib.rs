//! The compendium collection (spec §6.7). This milestone’s only entry is coin toss, as one bonus
//! capability the engine carries.

/// What an entry is: a bonus the engine carries, or a game written up in its own spec.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// A capability the engine carries, listed here. This milestone's only kind.
    Bonus,
    /// A game of its own, in its own spec. Not this milestone (§1).
    Core,
}

/// One entry of the collection. `capability` is a **listing**: the body, the launch its stream decides, the
/// step and the read-out are the engine's (Tasks 6, 7), never classics-core's (§6.7).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Classics {
    pub name: &'static str,
    pub kind: Kind,
    pub capability: &'static str,
}

/// The one entry, named. The type it names is what the coin is: a disc with thickness, two faces and an
/// edge (§6.1).
pub const COIN_TOSS: Classics = Classics {
    name: "coin-toss",
    kind: Kind::Bonus,
    capability: "padparadscha-table::CoinToss",
};

/// The collection as it stands this milestone: one entry. Dice and classics proper each get their own entry
/// in their own spec later (§1).
pub fn collection() -> &'static [Classics] {
    &[COIN_TOSS]
}
