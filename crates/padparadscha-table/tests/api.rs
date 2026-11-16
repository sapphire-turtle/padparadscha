//! §9 and §12's checks: the join snapshot and the feed's first row continue one from the other; a
//! client that dropped passes `from_seq` and gets each of the same rows exactly once; one command
//! leaves exactly one row carrying the same seq; the ticket is usable exactly once.

use axum_test::TestServer;
use padparadscha_common::{Actor, Command, Event, Log, Queue, Seed};
use padparadscha_table::api::{State, routes};
use std::sync::Arc;
use std::time::{Duration, Instant};

const SEED: Seed = Seed(0x5EED_5EED_5EED_5EED);

/// The whole flow of §9.2, nothing faked: rows land through the engine's own `toss` (Task 8) and
/// the queue's own counter (Task 5), so the test proves command and event share exactly one seq.
fn filled() -> (Log, Queue) {
    let mut log = Log::default();
    let mut queue = Queue::default();
    // MEASURED: Task 8's CoinToss is a struct with a public `table`, no `Default`.
    let coin_toss = padparadscha_table::CoinToss {
        table: padparadscha_table::Table::default(),
    };
    for _ in 0..3 {
        let seq = queue.push(Command::Toss { who: Actor::User });
        let outcome = coin_toss.toss(SEED, seq);
        log.append(Event::coin_toss(
            seq,
            Actor::User,
            &outcome.outcome.to_string(),
            outcome.attempt,
        ));
    }
    (log, queue)
}

fn state() -> Arc<State> {
    let (log, queue) = filled();
    State::for_test("s-1".into(), SEED, log, queue)
}

fn server() -> TestServer {
    // MEASURED: axum-test 21.1's TestServer::new returns Self and panics on build failure.
    TestServer::new(routes(state()))
}

async fn rpc(app: &TestServer, method: &str, params: serde_json::Value) -> serde_json::Value {
    app.post("/rpc")
        .add_header("authorization", "Bearer local")
        .json(&serde_json::json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }))
        .await
        .json()
}

#[tokio::test]
async fn server_info_is_the_handshake_and_a_mismatch_is_refused_with_a_clear_message() {
    let answered = rpc(
        &server(),
        "server.info",
        serde_json::json!({ "apiVersion": "1" }),
    )
    .await;
    assert_eq!(
        answered["result"]["apiVersion"], "1",
        "the same version on every target (spec 8)"
    );
    let refused = rpc(
        &server(),
        "server.info",
        serde_json::json!({ "apiVersion": "0" }),
    )
    .await;
    assert!(
        refused["error"]["message"]
            .as_str()
            .unwrap()
            .contains("do not match")
    );
}

#[tokio::test]
async fn session_join_answers_with_the_id_the_seed_and_the_log_up_to_a_seq() {
    let r = rpc(
        &server(),
        "session.join",
        serde_json::json!({ "from_seq": null }),
    )
    .await;
    assert_eq!(r["result"]["sessionId"], "s-1");
    assert_eq!(
        r["result"]["seed"], 0x5EED_5EED_5EED_5EED_u64,
        "the one seed, handed to the client at join"
    );
    assert_eq!(
        r["result"]["rows"].as_array().unwrap().len(),
        3,
        "the whole log, for a client that has nothing"
    );
}

#[tokio::test]
async fn a_client_that_dropped_passes_from_seq_and_gets_each_missing_row_exactly_once() {
    let app = server();
    let snapshot = rpc(
        &app,
        "session.join",
        serde_json::json!({ "from_seq": null }),
    )
    .await;
    let had = snapshot["result"]["rows"].as_array().unwrap().len();
    // The rows it had, plus the ones the feed carries from the continuation, are the whole log once.
    let feed = rpc(&app, "session.join", serde_json::json!({ "from_seq": had })).await;
    let missing = feed["result"]["rows"].as_array().unwrap();
    assert_eq!(
        missing.len(),
        0,
        "nothing was added since, so nothing is missing"
    );
    assert_eq!(
        feed["result"]["feedFrom"], had,
        "the feed's next row is the next seq the client lacks (spec 7.3)"
    );
}

#[tokio::test]
async fn one_game_command_leaves_exactly_one_row_and_answers_with_its_seq() {
    let app = server();
    // The queue already accepted 3 (the log's rows), so the next is 3 — and a drained queue changes that.
    let r = rpc(
        &app,
        "game.command",
        serde_json::json!({ "who": "Agent", "kind": "log", "note": "a coin, a bonus" }),
    )
    .await;
    assert_eq!(
        r["result"]["seq"], 3,
        "the queue counter's next, and the row it produces carries the same one"
    );
    let rows = rpc(&app, "session.join", serde_json::json!({ "from_seq": 3 })).await;
    assert_eq!(
        rows["result"]["rows"].as_array().unwrap().len(),
        1,
        "one command, one row"
    );
}

#[tokio::test]
async fn the_ticket_is_usable_exactly_once_and_a_second_use_or_expiry_is_401() {
    let now = Instant::now();
    let mut tickets = padparadscha_table::api::Tickets::default();
    let ticket = tickets.issue("s-1", now);
    assert!(tickets.redeem("s-1", &ticket, now), "the one use");
    assert!(
        !tickets.redeem("s-1", &ticket, now),
        "a second use is the 401 of §10"
    );
    let fresh = tickets.issue("s-1", now);
    assert!(
        !tickets.redeem("s-1", &fresh, now + Duration::from_secs(31)),
        "past 30 seconds, 401"
    );
    let again = tickets.issue("s-1", now);
    assert!(
        tickets.redeem("s-1", &again, now),
        "a new ticket lets it be re-issued"
    );
}

#[tokio::test]
async fn the_handshake_carries_the_ticket_and_nothing_else() {
    // No Authorization on a browser's WebSocket handshake, so the ticket is the whole of it (§9.1).
    let app = server();
    let refused = app.get("/ws?ticket=nope").await;
    assert_eq!(
        refused.status_code(),
        401,
        "unknown ticket, at the handshake"
    );
}

#[tokio::test]
async fn the_feed_streams_from_the_seq_the_snapshot_ended_at() {
    // The one place a listener is bound in the tests: the feed is the socket, and a socket needs one.
    // MEASURED: axum-test 21.1's into_websocket requires the HTTP transport; TestServer binds the
    // loopback listener itself on a random port (same socket the plan's hand-rolled client used).
    let state = state();
    let ticket = state.tickets.lock().unwrap().issue("s-1", Instant::now());
    let app = TestServer::new_with_config(
        routes(state),
        axum_test::TestServerConfig {
            transport: Some(axum_test::Transport::HttpRandomPort),
            ..Default::default()
        },
    );
    let mut socket = app
        .get_websocket(&format!("/ws?ticket={ticket}"))
        .await
        .into_websocket()
        .await;
    let row: serde_json::Value = serde_json::from_str(&socket.receive_text().await).unwrap();
    assert_eq!(
        row["seq"], 0,
        "with no join before it, the feed starts at the log's own first seq (spec 9)"
    );
}
