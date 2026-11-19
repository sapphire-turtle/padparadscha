//! §6.7 and §1: coin toss is one entry of the collection under `kind = bonus`, and the listing is
//! the whole of the classics-core side — physics and adjudication stay on the engine's side.

use padparadscha_classics_core::{Kind, collection};

#[test]
fn coin_toss_is_one_bonus_entry_and_carries_neither_physics_nor_adjudication() {
    let entries = collection();
    assert_eq!(
        entries.len(),
        1,
        "coin toss is a bonus, not another game being added (spec 1)"
    );
    let entry = &entries[0];
    assert_eq!(entry.name, "coin-toss");
    assert_eq!(
        entry.kind,
        Kind::Bonus,
        "a bonus capability the engine carries"
    );
    // What the entry names is the capability; the body, the read-out and the outcomes are the engine's.
    assert_eq!(entry.capability, "padparadscha-table::CoinToss");
    assert!(!entry.capability.contains("Outcome") && !entry.capability.contains("Launch"));
}
