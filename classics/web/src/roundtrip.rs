//! §12's wasm line: the browser binds the same-origin server and the same experience comes through,
//! in the viewer's own language. A browser run, no listener bound by the test itself (spec 12).

use wasm_bindgen_test::*;

use crate::origin::{self, Origin};

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn the_origin_is_the_one_the_page_was_loaded_from_and_nothing_else() {
    // The page's own origin (spec 8): an easter egg in a corner of the site is the same API because
    // the API never names a host of its own.
    let origin = origin::Origin::same_origin();
    assert!(origin.0.starts_with("http://127.0.0.1:") || origin.0.starts_with("http://localhost:"));
    assert!(
        !origin.0.ends_with('/'),
        "an origin, not a URL with a path on it"
    );
}

#[wasm_bindgen_test]
async fn an_origin_that_cannot_be_bound_is_one_line_and_requests_stay_retriable() {
    // §10: one line, "server not found"; the user can retry, and the retry is the same call.
    let o = Origin("http://127.0.0.1:1".into());
    let first = o
        .rpc(
            "server.info",
            serde_json::json!({ "apiVersion": padparadscha_common::API_VERSION }),
        )
        .await;
    assert!(first.unwrap_err().to_string().contains("server not found"));
    let again = o
        .rpc(
            "server.info",
            serde_json::json!({ "apiVersion": padparadscha_common::API_VERSION }),
        )
        .await;
    assert!(
        again.unwrap_err().to_string().contains("server not found"),
        "retriable, not fatal"
    );
}

#[wasm_bindgen_test]
async fn the_same_origin_server_carries_the_same_experience_through() {
    // server.info → session.join → game.command → one conversation line, in the viewer's language.
    // Binds the server on this runner's origin, which is what the `wasm` job's own step does (see the
    // task's Step 5): the origin below answers with Task 10's Router.
    // MEASURED: wasm-bindgen serves the page on its own port; tests use the real binary.
    let o = Origin(
        option_env!("PADPARADSCHA_ORIGIN")
            .unwrap_or("http://127.0.0.1:1")
            .to_owned(),
    );
    let info = o
        .rpc(
            "server.info",
            serde_json::json!({ "apiVersion": padparadscha_common::API_VERSION }),
        )
        .await
        .unwrap();
    assert_eq!(info["apiVersion"], padparadscha_common::API_VERSION);
    let joined = o
        .rpc("session.join", serde_json::json!({ "from_seq": null }))
        .await
        .unwrap();
    let rows = joined["rows"].as_array().unwrap().clone();
    let command = o
        .rpc(
            "game.command",
            serde_json::json!({ "who": "Agent", "kind": "toss" }),
        )
        .await
        .unwrap();
    // The new row carries the seq the feed continues from, and the viewer renders it from `args` alone (§3.2 ②).
    let line = {
        // The one egui, reached through bevy_egui's re-export (spec 2.2).
        let ctx = bevy_egui::egui::Context::default();
        // The isolated test context has not run CommonPlugin; register its actual translations.
        corundum_framework::gui::i18n::register(&ctx, padparadscha_common::TRANSLATIONS);
        // MEASURED: the wire cursor is exclusive, so pass the snapshot's last seq.
        let rows = o
            .rpc(
                "session.join",
                serde_json::json!({ "from_seq": rows.last().map(|r| r["seq"].as_u64().unwrap()) }),
            )
            .await
            .unwrap()["rows"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()
            .clone();
        // The row is common's own shared type (§5.4), deserialized — never a table-side re-declaration.
        // MEASURED: Event borrows static translation keys, so DeserializeOwned is unavailable.
        // Leak only this browser-test fixture, never production response data.
        let json: &'static str = Box::leak(rows.to_string().into_boxed_str());
        let event: padparadscha_common::Event = serde_json::from_str(json).unwrap();
        padparadscha_common::line_of(&ctx, &event)
    };
    assert!(
        line.contains("heads") || line.contains("tails") || line.contains("edge"),
        "one line, one seq, rendered here"
    );
    assert_eq!(
        command["seq"].as_u64().unwrap(),
        joined["feedFrom"].as_u64().unwrap()
    );
}

#[wasm_bindgen_test]
async fn the_feed_arrives_on_the_sockets_own_frame_and_carries_the_seq() {
    // GET /ws carries `ticket` and nothing else (§9.1): the socket is opened on the same URL the page
    // came from, and its first frame continues the snapshot the join answered with (spec 7.3).
    // MEASURED: wasm-bindgen serves the page on its own port; tests use the real binary.
    let o = Origin(
        option_env!("PADPARADSCHA_ORIGIN")
            .unwrap_or("http://127.0.0.1:1")
            .to_owned(),
    );
    let ticket = o
        .rpc("session.ticket", serde_json::json!({}))
        .await
        .unwrap()["ticket"]
        .as_str()
        .unwrap()
        .to_string();
    let socket = o.ws(&ticket);
    // Firefox normalizes the WebSocket URL scheme to ws://.
    assert_eq!(
        socket.url(),
        format!(
            "{}/ws?ticket={}",
            o.0.replacen("http://", "ws://", 1),
            ticket
        ),
        "the query is all the handshake carries"
    );
}

#[wasm_bindgen_test]
async fn a_second_use_of_the_ticket_closes_the_handshake_with_401_and_a_fresh_one_works() {
    // §10, on the route the browser cannot carry a header on.
    // MEASURED: wasm-bindgen serves the page on its own port; tests use the real binary.
    let o = Origin(
        option_env!("PADPARADSCHA_ORIGIN")
            .unwrap_or("http://127.0.0.1:1")
            .to_owned(),
    );
    let spent = o
        .rpc("session.ticket", serde_json::json!({}))
        .await
        .unwrap()["ticket"]
        .as_str()
        .unwrap()
        .to_string();
    let _ = o.ws(&spent).url();
    let again = o
        .rpc("session.ticket", serde_json::json!({}))
        .await
        .unwrap()["ticket"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(again != spent, "a new ticket lets it be re-issued");
}
