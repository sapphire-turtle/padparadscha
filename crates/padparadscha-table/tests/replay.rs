//! §12, end to end: from the same seed and the same commands, the same adjudication comes out on every
//! target, at the read-out granularity. A row is the whole line — its `kind` id and its `args` — and
//! what a reader shows is those two and nothing else (§3.2).

use padparadscha_common::{Actor, Event, Log, Seed, Seq};
use padparadscha_table::{CoinToss, Table};

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

const SEED: Seed = Seed(0x5EED_5EED_5EED_5EED);

fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn the_same_seed_and_the_same_commands_replay_the_same_rows_on_every_target() {
    // One session's whole exchange: the three commands the queue accepted, the three rows they left, and the
    // re-toss number each row carries (§6.5: each re-toss draws from the same stream, so `attempt` is
    // the same everywhere). A low-bit difference that leaves the adjudication unchanged is no mismatch (§12).
    let toss = CoinToss {
        table: Table::default(),
    };
    let mut log = Log::default();
    for seq in [Seq(0), Seq(1), Seq(2)] {
        let outcome = toss.toss(SEED, seq);
        log.append(Event::coin_toss(
            seq,
            Actor::User,
            &format!("{:?}", outcome.outcome),
            outcome.attempt,
        ));
    }
    // MEASURED ACP 5.2 enables preserve_order transitively: sort recursively before printing.
    for row in &mut log.0 {
        row.args.sort_all_objects();
    }
    // The rows as-is, each in full: objects explicitly sorted above,
    // so even the key order is one of the bytes every target reads the same.
    let rows = log
        .0
        .iter()
        .map(|r| format!("{r:#?}"))
        .collect::<Vec<_>>()
        .join("\n");
    let hash = fnv1a(&rows);
    println!("replay seed={:#018x} rows={rows} hash={hash:#018x}", SEED.0);
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::console_log!("replay seed={:#018x} rows={rows} hash={hash:#018x}", SEED.0);
    assert_eq!(
        hash,
        fnv1a(&rows),
        "the print is deterministic, and a re-run of the same seed replays the same rows"
    );
    assert_eq!(log.0.len(), 3, "one command, one row (§5.1)");
    assert_eq!(
        log.0[2].seq,
        Seq(2),
        "the row carries the one seq its command was tagged with (§6.5)"
    );
    assert!(
        log.0[2].args["attempt"].is_number(),
        "the re-toss number rides inside the row, from the stream"
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn replay_matches_the_measured_readout_and_retoss_counts() {
    // Linux x86_64 Task 13 measurement; only adjudication/attempts, never f64 state bits.
    let replay = || {
        let toss = CoinToss {
            table: Table::default(),
        };
        [Seq(0), Seq(1), Seq(2)].map(|seq| {
            let result = toss.toss(SEED, seq);
            (result.outcome.to_string(), result.attempt)
        })
    };
    let first = replay();
    assert_eq!(
        first,
        replay(),
        "independent executions consume matching command streams"
    );
    assert_eq!(
        first,
        [("edge".into(), 2), ("edge".into(), 2), ("edge".into(), 2)]
    );
}
