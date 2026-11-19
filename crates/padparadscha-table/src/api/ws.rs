//! `GET /ws?ticket=…`: the feed (spec §9). It carries `ticket` and nothing else (§9.1); once
//! the handshake is answered, it streams log appends and conversation content, from the seq the
//! snapshot ended at (§7.3's gapless continuation).

use crate::api::State as Engine;
use axum::extract::{Query, State, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use padparadscha_common::Seq;
use std::sync::Arc;

pub async fn ws_upgrade(
    State(state): State<Arc<Engine>>,
    // MEASURED (axum 0.8.9): as a bare extractor, WebSocketUpgrade rejects a non-upgrade GET with
    // 400 BEFORE the ticket is read. §10's contract is that an unknown/expired/second-use ticket is
    // 401 at the handshake, so the ticket is redeemed first and the upgrade rejection is deferred.
    ws: Result<WebSocketUpgrade, axum::extract::ws::rejection::WebSocketUpgradeRejection>,
    Query(q): Query<std::collections::HashMap<String, String>>,
) -> axum::response::Response {
    // Unknown, expired or a second use: 401 at the handshake, and the screen fetches a new ticket
    // and reconnects (§10). Nothing else rides on this handshake.
    let ticket = q.get("ticket").cloned().unwrap_or_default();
    if !state.tickets.lock().unwrap().redeem(
        &state.session.lock().unwrap().id,
        &ticket,
        state.now(),
    ) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    match ws {
        Ok(ws) => ws
            .on_upgrade(move |socket| feed(socket, state))
            .into_response(),
        Err(rejection) => rejection.into_response(),
    }
}

/// Every row from the seq the snapshot ended at (with no join before, the log's own first), as-is and
/// structured, one line per text frame — the shape of the peer's own `Lines` transport: a frame *is*
/// a message. What is appended after continues on the same stream, so a client that dropped misses nothing.
// MEASURED (axum 0.8.9): the socket is `axum::extract::ws::WebSocket` and frames are
// `axum::extract::ws::Message` — the plan's `axum::extract::private::*` paths do not exist.
async fn feed(socket: axum::extract::ws::WebSocket, state: Arc<Engine>) {
    let mut sink = socket;
    let mut from = state.feed_from.lock().unwrap().unwrap_or(Seq(0));
    loop {
        let rows = state.log.lock().unwrap().since(from);
        // Rows appended between joining and this drain still belong to the finite replay.
        // No idle notification is needed: when no row is available this replay is drained.

        // Task 13 ruling: replay is finite; rejoin with a new ticket for later appends.
        if rows.is_empty() {
            let _ = sink.send(axum::extract::ws::Message::Close(None)).await;
            return;
        }
        for row in rows {
            let frame = serde_json::to_string(&row).unwrap();
            if sink
                .send(axum::extract::ws::Message::Text(frame.into()))
                .await
                .is_err()
            {
                return;
            }
            from = Seq(row.seq.0 + 1);
        }
    }
}
