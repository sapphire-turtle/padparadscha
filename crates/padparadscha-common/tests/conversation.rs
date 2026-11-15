//! §5.1 and §5.4: both engines share one command/event seq and the queue's monotonic
//! counter; §7.3: a joining peer replays an inclusive snapshot and a gapless continuation.

use padparadscha_common::{Actor, Command, Event, Log, Queue, Seq};
use serde_json::json;

#[test]
fn the_queue_owns_a_monotonic_counter_even_after_earlier_rows_drained() {
    let mut queue = Queue::default();
    assert_eq!(queue.next, Seq(0));
    let command = Command::Toss { who: Actor::User };
    let seq = queue.push(command.clone());
    assert_eq!(seq, Seq(0), "push returns the queue's own next seq (§5.1)");
    assert_eq!(queue.take_all().collect::<Vec<_>>(), [(seq, command)]);
    assert!(queue.lines.is_empty());
    assert_eq!(queue.next, Seq(1));

    let command = Command::Toss { who: Actor::Agent };
    let seq = queue.push(command.clone());
    let rows = queue.take_all().collect::<Vec<_>>();
    let event = Event::coin_toss(rows[0].0, Actor::Agent, "heads", 1);
    assert_eq!(seq, Seq(1), "draining does not reset the counter");
    assert_eq!(rows, [(seq, command)]);
    assert_eq!(
        event.seq, seq,
        "one command and its event share exactly one seq (§5.1)"
    );
    assert_eq!(queue.next, Seq(2));
}

#[test]
fn take_all_drains_commands_in_the_order_they_arrived() {
    let mut queue = Queue::default();
    let toss = Command::Toss { who: Actor::User };
    let note = Command::Log {
        note: "the coin, for a choice".into(),
    };
    let first = queue.push(toss.clone());
    let second = queue.push(note.clone());
    assert_eq!(
        queue.take_all().collect::<Vec<_>>(),
        [(first, toss), (second, note)]
    );
    assert!(
        queue.lines.is_empty(),
        "take_all drains, rather than copies"
    );
    assert_eq!(queue.take_all().count(), 0);
    assert_eq!(queue.next, Seq(2));
}

#[test]
fn continuing_from_relinks_the_snapshot_and_a_gapless_replay() {
    let mut log = Log::default();
    log.append(Event::coin_toss(Seq(0), Actor::User, "heads", 1));
    log.append(Event::coin_toss(Seq(1), Actor::Agent, "tails", 2));
    let snapshot = log.until(Seq(1));
    let next = Seq(snapshot.last().unwrap().seq.0 + 1);
    let mut queue = Queue::continuing_from(next);
    assert!(queue.lines.is_empty());
    assert_eq!(queue.next, Seq(2));
    assert_eq!(queue.push(Command::Toss { who: Actor::User }), next);
    assert_eq!(queue.push(Command::Toss { who: Actor::Agent }), Seq(3));
    for (seq, command) in queue.take_all() {
        let Command::Toss { who } = command else {
            panic!("a queued toss")
        };
        log.append(Event::coin_toss(seq, who, "heads", 1));
    }
    let mut replay = snapshot;
    replay.extend(log.since(next));
    assert_eq!(
        replay, log.0,
        "snapshot and continuation leave no gap or duplicate (§7.3)"
    );
    assert_eq!(
        replay.iter().map(|row| row.seq).collect::<Vec<_>>(),
        [Seq(0), Seq(1), Seq(2), Seq(3)]
    );
    assert_eq!(queue.next, Seq(4));
}

#[test]
fn append_until_and_since_window_the_same_rows_inclusively() {
    let mut log = Log::default();
    assert!(log.until(Seq(1)).is_empty());
    assert!(log.since(Seq(1)).is_empty());
    let rows = [
        Event::of(Seq(1), "table.note", json!({ "note": "the coin" })),
        Event::coin_toss(Seq(2), Actor::User, "heads", 1),
        Event::coin_toss(Seq(3), Actor::Agent, "tails", 2),
    ];
    for row in &rows {
        log.append(row.clone());
    }
    assert_eq!(log.0, rows, "append preserves each row and its seq");
    assert_eq!(
        log.until(Seq(2)),
        rows[..2],
        "the snapshot includes its upper seq"
    );
    assert_eq!(
        log.since(Seq(2)),
        rows[1..],
        "the feed includes its starting seq"
    );
    assert!(log.until(Seq(0)).is_empty());
    assert!(log.since(Seq(4)).is_empty());
    assert_eq!(log.until(Seq(4)), rows);
    assert_eq!(log.since(Seq(0)), rows);
    assert_eq!(log.0, rows, "reading windows never rewrites the log");
}

#[test]
fn an_event_carries_its_fluent_id_and_structured_args_without_rendering() {
    let args = json!({ "note": "the coin", "reason": "a choice", "detail": [1, 2] });
    let event = Event::of(Seq(7), "table.note", args.clone());
    assert_eq!(event.seq, Seq(7));
    assert_eq!(
        event.kind, "table.note",
        "kind is a Fluent message id (§3.1)"
    );
    assert_eq!(event.args, args);
    for who in [Actor::User, Actor::Agent] {
        let event = Event::coin_toss(Seq(8), who, "tails", 3);
        assert_eq!(event.seq, Seq(8));
        assert_eq!(event.kind, "coin-toss.outcome");
        assert_eq!(
            event.args,
            json!({ "who": who, "outcome": "tails", "attempt": 3 }),
            "§6.6's args stay structured for either reader"
        );
    }
}
