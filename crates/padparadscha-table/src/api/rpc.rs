//! `POST /rpc`: JSON-RPC 2.0 (spec §9). The params and results are structured as-is (§3.2):
//! turning a row into text is the reader's job, never this layer's.

use crate::api::{State, ws};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use padparadscha_common::{Actor, Command, Event, Seq};
use serde_json::Value;
use std::sync::Arc;

/// The one API every target mounts (spec §8): `POST /rpc`, and `GET /ws` for the feed.
pub fn routes(state: Arc<State>) -> Router {
    Router::new()
        .route("/rpc", post(rpc_post))
        .route("/ws", axum::routing::get(ws::ws_upgrade))
        .layer(axum::middleware::from_fn(loopback_cors))
        .with_state(state)
}

async fn rpc_post(
    axum::extract::State(state): axum::extract::State<Arc<State>>,
    Json(req): Json<Value>,
) -> impl IntoResponse {
    let method = req["method"].as_str().unwrap_or_default().to_string();
    let params = req.get("params").cloned().unwrap_or(Value::Null);
    let answered = match method.as_str() {
        // ① The handshake: API version in, clear message or the version out (Task 4's check).
        // Task 4's handshake lives in common; table imports it, never re-declares it (§5.4).
        "server.info" => padparadscha_common::check_api_version(
            params["apiVersion"].as_str().unwrap_or_default(),
        )
        .map(|()| serde_json::json!({ "apiVersion": padparadscha_common::API_VERSION }))
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string())),
        // ② The snapshot. `from_seq` is an argument of this method alone (§9.1).
        "session.join" => Ok(join(&state, params)),
        // ③ The one-time ticket, bound to the session id (this task's ticket).
        "session.ticket" => Ok(serde_json::json!({
            "ticket": state.tickets.lock().unwrap().issue(&state.session.lock().unwrap().id, state.now()),
        })),
        // ④ game.command and ⑤ chat.send, on the queue and the log of Task 5.
        "game.command" => Ok(command(&state, params)),
        "chat.send" => Ok(chat(&state, params)),
        _ => Err(mismatch(method)),
    };
    answer(&req, answered)
}

/// The snapshot, and the seq the feed continues from: `from_seq` is the next seq the client has not
/// received; with nothing passed, the whole log, from nothing (spec §7.3).
fn join(state: &Arc<State>, params: Value) -> Value {
    let from = params.get("from_seq").and_then(Value::as_u64).map(Seq);
    let snapshot = {
        let session = state.session.lock().unwrap();
        let log = state.log.lock().unwrap();
        // Task 9's resolved contract: the snapshot carries the actual bound session id, not the
        // settings-only compatibility `Join::join` (which has no id to give).
        padparadscha_common::Join::for_session(session.id.clone(), session.seed, &log, from)
    };
    let feed_from = snapshot
        .rows
        .last()
        .map(|row| Seq(row.seq.0 + 1))
        .or(from)
        .unwrap_or(Seq(0));
    *state.feed_from.lock().unwrap() = Some(feed_from);
    serde_json::json!({
        "sessionId": snapshot.id,
        "seed": snapshot.seed,
        "rows": snapshot.rows,
        "feedFrom": feed_from.0,
    })
}

/// ④ `game.command`: accepted at the end of the queue with the next seq, and what it produces is one
/// row carrying that same seq. A command arriving while a simulation runs waits its turn (§10).
fn command(state: &Arc<State>, params: Value) -> Value {
    let who = serde_json::from_value::<Actor>(params["who"].clone()).unwrap();
    let command = match params["kind"].as_str().unwrap_or_default() {
        "toss" => Command::Toss { who },
        _ => Command::Log {
            note: params["note"].as_str().unwrap_or_default().to_string(),
        },
    };
    let seq = state.queue.lock().unwrap().push(command);
    let seed = state.session.lock().unwrap().seed;
    let outcome = state.coin_toss.toss(seed, seq);
    state.log.lock().unwrap().append(Event::coin_toss(
        seq,
        who,
        &outcome.outcome.to_string(),
        outcome.attempt,
    ));
    state.appended.notify_waiters();
    serde_json::json!({ "seq": seq.0 })
}

/// ⑤ `chat.send`: an utterance reaches the game server here and becomes the ACP user message (§4.2).
fn chat(state: &Arc<State>, params: Value) -> Value {
    let text = params["text"].as_str().unwrap_or_default().to_string();
    let seq = state
        .queue
        .lock()
        .unwrap()
        .push(Command::Log { note: text });
    serde_json::json!({ "seq": seq.0 })
}

/// A JSON-RPC 2.0 answer: `result` on the one hand, `error` on the other, and the id it answers.
// MEASURED: Rust 2024's impl-Trait capture would tie the response to `req`'s lifetime (E0515),
// so the concrete Response type is named instead of `impl IntoResponse`.
fn answer(req: &Value, answered: Result<Value, (StatusCode, String)>) -> axum::response::Response {
    let id = req.get("id").cloned().unwrap_or(Value::Null);
    match answered {
        Ok(result) => (
            StatusCode::OK,
            Json(serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": result })),
        )
            .into_response(),
        Err((code, message)) => (
            code,
            Json(
                serde_json::json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32000, "message": message } }),
            ),
        )
            .into_response(),
    }
}

fn mismatch(method: String) -> (StatusCode, String) {
    (
        StatusCode::NOT_FOUND,
        format!("no method {method:?} on this API"),
    )
}

// Browser test runner binds a different loopback port; only loopback origins may cross ports.
async fn loopback_cors(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let origin = request.headers().get(axum::http::header::ORIGIN).cloned();
    let allowed = origin
        .as_ref()
        .and_then(|v| v.to_str().ok())
        .is_some_and(|value| {
            value
                .strip_prefix("http://127.0.0.1:")
                .or_else(|| value.strip_prefix("http://localhost:"))
                .is_some_and(|port| port.parse::<u16>().is_ok())
        });
    let mut response = if allowed && request.method() == axum::http::Method::OPTIONS {
        StatusCode::NO_CONTENT.into_response()
    } else {
        next.run(request).await
    };
    if let Some(origin) = origin.filter(|_| allowed) {
        response
            .headers_mut()
            .insert(axum::http::header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
        response.headers_mut().insert(
            axum::http::header::ACCESS_CONTROL_ALLOW_METHODS,
            "POST, OPTIONS".parse().unwrap(),
        );
        response.headers_mut().insert(
            axum::http::header::ACCESS_CONTROL_ALLOW_HEADERS,
            "content-type, authorization".parse().unwrap(),
        );
        response
            .headers_mut()
            .insert(axum::http::header::VARY, "origin".parse().unwrap());
    }
    response
}
