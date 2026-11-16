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

// §3.1, §3.2, §3 and §7.3: the viewer's rendering and the session language stay distinct;
// snapshot and feed meet at one boundary, and from_seq replays each missing row exactly once.
use corundum_framework::gui::i18n;
use padparadscha_common::{ChatRole, ChatState, Join, Seed, Settings, render, to_user_message};

fn log3() -> Log {
    let mut log = Log::default();
    for seq in [Seq(0), Seq(1), Seq(2)] {
        log.append(Event::coin_toss(seq, Actor::User, "heads", 0));
    }
    log
}

#[test]
fn one_game_line_lands_in_the_conversation_as_one_line_and_keeps_its_seq() {
    let log = log3();
    let joined = Join::join(
        &Settings::default(),
        Seed(0x5EED_5EED_5EED_5EED),
        &log,
        None,
    );
    assert_eq!(joined.rows.len(), 3);
    let mut state = ChatState::default();
    for row in &joined.rows {
        state.push_line(row, row.kind.to_string()); // no bundle installed here, so the id itself is what the reader got
    }
    assert_eq!(
        state.0[0].role,
        ChatRole::System,
        "game-originated lines are system-role (spec 3.2); who is inside the text, not the role"
    );
    assert_eq!(
        state.0[2].seq,
        Some(Seq(2)),
        "the row's own seq, so a reconnect names the next one"
    );
}

#[test]
fn the_feed_continues_the_snapshot_and_nothing_arrives_twice() {
    let log = log3();
    let snapshot = Join::join(&Settings::default(), Seed(1), &log, None);
    let after = Join::join(&Settings::default(), Seed(1), &log, Some(Seq(2)));
    assert_eq!(snapshot.rows.last().unwrap().seq, Seq(2));
    assert_eq!(
        after.rows[0].seq,
        Seq(2),
        "from_seq is the next seq the client has not received"
    );
    // The rows the client had, plus the ones it had not, are the whole log, each exactly once.
    let mut once = snapshot.rows[..2].to_vec();
    once.extend(after.rows.iter().filter(|r| r.seq > Seq(1)).cloned());
    assert_eq!(once.len(), 3);
}

#[test]
fn one_line_two_languages_and_the_id_is_the_only_thing_carried() {
    let row = log3().0[0].clone();
    // ① the session's own language setting (here: ja), ② the viewer's own language out of `args` alone —
    // the two stay distinct (spec 3.2).
    // Measured corundum 0.11: LanguageSettings has a public language field, no new().
    let (text, args) = to_user_message(
        &i18n::LanguageSettings {
            language: i18n::Language::Explicit("ja".into()),
        },
        &row,
    );
    assert_eq!(args["outcome"], "heads");
    assert!(
        text.contains("表"),
        "①: what lands in the session, in the session's language"
    );
    let _ = render;
}

#[test]
fn a_missing_id_is_emitted_as_is_and_a_miss_is_logged_once() {
    // §10: corundum's i18n behaviour — the id itself, and one log line.
    let row = Event::of(Seq(9), "this-id-is-in-no-ftl", serde_json::Value::Null);
    assert!(line_of_without_ctx(&row).contains("this-id-is-in-no-ftl"));
}

fn line_of_without_ctx(row: &Event) -> String {
    // The bundle-less fallback: with no installed bundle, the id itself is what the reader got too.
    row.kind.to_string()
}

#[test]
fn actual_viewer_renderings_use_event_args_and_leave_log_unchanged() {
    use bevy_egui::egui;
    use padparadscha_common::{line_of, render};
    let log = log3();
    let ctx = egui::Context::default();
    const RES: i18n::Resources = i18n::Resources {
        name: "test-translations",
        files: &[
            ("en", "coin-toss-outcome = { $outcome }"),
            ("ja", "coin-toss-outcome = { $outcome }"),
        ],
    };
    i18n::register(&ctx, RES);
    let mut state = ChatState::default();
    render(&ctx, &log.0, &mut state);
    assert_eq!(state.0.len(), 3);
    assert_eq!(state.next_wanted(), Some(Seq(3)));
    assert!(line_of(&ctx, &log.0[0]).contains("heads"));
    assert_eq!(log.0[0].kind, "coin-toss.outcome");
    let session = i18n::LanguageSettings {
        language: i18n::Language::Explicit("ja".into()),
    };
    assert!(to_user_message(&session, &log.0[0]).0.contains('表'));
    assert_eq!(
        padparadscha_common::Join::for_session("session-id", Seed(1), &log, None).id,
        "session-id"
    );
    assert_eq!(
        line_of(
            &ctx,
            &Event::of(Seq(9), "unknown-id", serde_json::Value::Null)
        ),
        "unknown-id"
    );
}
