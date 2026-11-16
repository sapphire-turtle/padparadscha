//! §5.1's table and session — the engine's own types. The queue, the log and their tests are
//! common's (Task 5): this file's job is proving the engine consumes them without re-declaring them.

use padparadscha_common::{Actor, Command, Event, Log, Queue, Seed};
use padparadscha_table::{Body, Session, Table};

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn a_table_is_its_dimensions_edge_and_wall_and_its_normal_is_what_the_angle_is_taken_against() {
    let table = Table::default();
    assert_eq!(
        table.normal(),
        [0.0, 1.0, 0.0],
        "the read-out's second side (spec 6.4)"
    );
    assert_eq!(table.half_width_m, 0.9);
    assert_eq!(table.half_depth_m, 0.6);
    assert_eq!(table.wall_m, 0.02);
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn one_session_makes_one_seed_and_holds_the_bodies_it_names() {
    let session = Session::new("s-1", Seed(0x5EED_5EED_5EED_5EED));
    assert_eq!(
        session.seed.0, 0x5EED_5EED_5EED_5EED,
        "one per session, made once (spec 7.1)"
    );
    let seed: Seed = session.seed;
    assert_eq!(seed, Seed(0x5EED_5EED_5EED_5EED));
    assert_eq!(session.bodies, [Body::Coin]);
    assert_eq!(session.id, "s-1");
    let mut app = bevy::prelude::App::default();
    app.insert_resource(session);
    assert_eq!(app.world().resource::<Session>().seed, seed);
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn the_engine_holds_common_types_as_its_own_state_without_redeclaring_them() {
    // §5.4: the queue and the log are not re-exported from here, because they are common's types.
    let mut app = bevy::prelude::App::default();
    app.insert_resource(padparadscha_common::Log::default());
    let log = app.world().resource::<padparadscha_common::Log>();
    assert!(
        log.0.is_empty(),
        "the engine's state holds the very same type common declares"
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn queued_commands_and_events_share_the_same_seq() {
    let mut queue = Queue::default();
    let mut log = Log::default();
    let first = queue.push(Command::Toss { who: Actor::User });
    let second = queue.push(Command::Toss { who: Actor::Agent });
    for (seq, command) in queue.take_all() {
        let Command::Toss { who } = command else {
            panic!("expected toss")
        };
        log.append(Event::coin_toss(seq, who, "heads", 0));
    }
    assert_ne!(first, second);
    assert_eq!(
        log.0.iter().map(|event| event.seq).collect::<Vec<_>>(),
        [first, second]
    );
    assert_eq!(log.0[0].args["who"], "User");
    assert_eq!(log.0[1].args["who"], "Agent");
}
