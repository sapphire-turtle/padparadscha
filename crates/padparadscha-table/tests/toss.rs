//! §6.4, §6.5, §12's read-out test (the three cases split exactly as 6.4 says) and §10's
//! "the queue itself is not blocked".

use padparadscha_common::{Actor, Command, Event, Log, Queue, Seed, Seq};
use padparadscha_table::{CoinToss, Outcome, Table};

const SEED: Seed = Seed(0x5EED_5EED_5EED_5EED);

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
#[expect(
    clippy::approx_constant,
    reason = "the measured edge fixture is deliberately approximate"
)]
fn the_three_cases_split_exactly_as_the_read_out_says() {
    // under 45° is that face; 45° or more cannot be adjudicated (spec 6.4).
    let toss = CoinToss {
        table: Table::default(),
    };
    let up = [0.0_f64, 1.0, 0.0];
    assert_eq!(toss.adjudicate(toss.face_cos(up, up)), Some(Outcome::Heads));
    assert_eq!(
        toss.adjudicate(toss.face_cos(CoinToss::FACE_B_WORLD, up)),
        Some(Outcome::Tails)
    );
    // Standing on the edge: no adjudication, so it re-tosses on its own.
    assert_eq!(toss.adjudicate(toss.face_cos([1.0, 0.0, 0.0], up)), None);
    assert_eq!(
        toss.adjudicate(toss.face_cos([0.707_106_8, 0.707_106_8, 0.0], up)),
        None
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn a_re_toss_makes_no_new_row_and_the_attempt_number_carries_it() {
    let toss = CoinToss {
        table: Table::default(),
    };
    let mut queue = Queue::default();
    let mut log = Log::default();
    let seq = queue.push(Command::Toss { who: Actor::User });
    let (_, Command::Toss { who }) = queue.take_all().next().unwrap() else {
        panic!("the queued command is a toss");
    };
    let outcome = toss.toss(SEED, seq);
    log.append(Event::coin_toss(
        seq,
        who,
        &format!("{:?}", outcome.outcome),
        outcome.attempt,
    ));
    assert!(outcome.attempt <= 2, "0 ≤ attempt ≤ 2 (spec 6.5)");
    // One command, one row, and the attempt sits inside it.
    assert_eq!(log.0.len(), 1);
    assert_eq!(log.0[0].args["attempt"], outcome.attempt);
    // The stream is one per command, so the same (seed, seq) gives the same attempt on every target.
    assert_eq!(outcome, toss.toss(SEED, seq));
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn an_edge_and_a_queued_command_do_not_block_the_queue() {
    // §10: other peers' commands pass through as-is, queued commands stay queued, and what is
    // decided is only "the user tossed once more and an outcome came out".
    let toss = CoinToss {
        table: Table::default(),
    };
    let mut queue = Queue::default();
    let stuck = queue.push(Command::Toss { who: Actor::User });
    let other = queue.push(Command::Toss { who: Actor::Agent });
    let both: Vec<(Seq, Command)> = queue.take_all().collect();
    assert_eq!(both[0].0, stuck);
    assert_eq!(both[1].0, other);
    let edge = toss.toss(SEED, stuck);
    assert!(
        edge.attempt == 2
            && matches!(
                edge.outcome,
                Outcome::Heads | Outcome::Tails | Outcome::Edge
            )
    );
}
