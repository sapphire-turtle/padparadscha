//! Host window adapters: one client joins and follows the same API as the browser.
use crate::{Actor, ChatRole, ChatState, Event, Seq};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use futures_util::StreamExt;
use serde_json::{Value, json};
use std::{
    net::SocketAddr,
    sync::{Mutex, mpsc},
};

/// The loopback client and its asynchronous connection driver, owned by the window.
#[derive(Resource)]
pub struct Client {
    addr: SocketAddr,
    runtime: tokio::runtime::Runtime,
    input: String,
    outgoing: Option<String>,
    rows: Mutex<mpsc::Receiver<Value>>,
}

impl Client {
    /// Join the actual bound server, then follow its ticketed feed.
    pub fn new(addr: SocketAddr) -> anyhow::Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        let (tx, rows) = mpsc::channel();
        runtime.spawn(async move {
            let mut cursor = None;
            loop {
                let result: anyhow::Result<()> = async {
                    let joined = rpc(addr, "session.join", json!({"from_seq": cursor})).await?;
                    let rows = joined["rows"]
                        .as_array()
                        .ok_or_else(|| anyhow::anyhow!("missing snapshot rows"))?;
                    for row in rows {
                        deliver(&tx, &mut cursor, row.clone())?;
                    }
                    let ticket = rpc(addr, "session.ticket", json!({})).await?;
                    let ticket = ticket["ticket"]
                        .as_str()
                        .ok_or_else(|| anyhow::anyhow!("missing ticket"))?;
                    let (mut socket, _) =
                        tokio_tungstenite::connect_async(format!("ws://{addr}/ws?ticket={ticket}"))
                            .await?;
                    while let Some(frame) = socket.next().await {
                        let frame = frame?;
                        if frame.is_text() {
                            deliver(&tx, &mut cursor, serde_json::from_str(frame.to_text()?)?)?;
                        } else if frame.is_close() {
                            break;
                        }
                    }
                    Ok(())
                }
                .await;
                if let Err(error) = result {
                    tracing::warn!(%error, "chat connection closed; retrying");
                }
                // A drained feed is finite. Avoid a busy rejoin loop while waiting for new rows.
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            }
        });
        Ok(Self {
            addr,
            runtime,
            input: String::new(),
            outgoing: None,
            rows: Mutex::new(rows),
        })
    }

    fn request(&self, method: &'static str, params: Value) {
        let addr = self.addr;
        self.runtime.spawn(async move {
            if let Err(error) = rpc(addr, method, params).await {
                tracing::warn!(%error, method, "chat request failed");
            }
        });
    }
}

// Both the snapshot and the feed advance the same exclusive wire cursor, including unseen kinds.
fn deliver(tx: &mpsc::Sender<Value>, cursor: &mut Option<u64>, row: Value) -> anyhow::Result<()> {
    let seq = row["seq"]
        .as_u64()
        .ok_or_else(|| anyhow::anyhow!("missing row seq"))?;
    if cursor.is_none_or(|last| seq > last) {
        tx.send(row)?;
        *cursor = Some(seq);
    }
    Ok(())
}

async fn rpc(addr: SocketAddr, method: &str, params: Value) -> anyhow::Result<Value> {
    let answer: Value = reqwest::Client::new()
        .post(format!("http://{addr}/rpc"))
        .json(&json!({"jsonrpc":"2.0", "id":1, "method":method, "params":params}))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    if let Some(error) = answer.get("error") {
        anyhow::bail!("{error}");
    }
    answer
        .get("result")
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("missing result"))
}

/// Render snapshot/feed lines and the input panel using the viewer's active i18n context.
pub fn render(
    mut contexts: EguiContexts,
    mut client: ResMut<Client>,
    mut state: ResMut<ChatState>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    let rows: Vec<_> = client.rows.lock().unwrap().try_iter().collect();
    for row in rows {
        let Some(seq) = row["seq"].as_u64() else {
            continue;
        };
        if row["kind"] != "coin-toss.outcome"
            || state.next_wanted().is_some_and(|next| seq < next.0)
        {
            continue;
        }
        let event = Event::of(Seq(seq), "coin-toss.outcome", row["args"].clone());
        state.push_line(&event, super::line_of(ctx, &event));
    }
    let mut viewport = egui::Ui::new(
        ctx.clone(),
        egui::Id::unique("chat-viewport"),
        egui::UiBuilder::new()
            .layer_id(egui::LayerId::background())
            .max_rect(ctx.viewport_rect()),
    );
    client.outgoing = super::chat_panel(&mut viewport, &state, &mut client.input);
}

/// Send the utterance collected by the panel and show it in the conversation.
pub fn send(mut client: ResMut<Client>, mut state: ResMut<ChatState>) {
    if let Some(text) = client.outgoing.take() {
        client.request("chat.send", json!({"text":text}));
        state.push(ChatRole::User, text);
    }
}

/// A narrow button issues exactly the same command as a remote client.
pub fn toss_button(mut contexts: EguiContexts, client: Res<Client>) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    egui::Window::new("Coin toss").show(ctx, |ui| {
        if ui.button("Toss coin").clicked() {
            client.request("game.command", json!({"who":Actor::User, "kind":"toss"}));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Json, Router, extract::State, routing::post};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    #[derive(Default)]
    struct Feed {
        rows: Vec<Value>,
        from: u64,
        joins: Vec<Option<u64>>,
        tickets: usize,
        sockets: usize,
    }

    async fn request(State(state): State<Arc<Mutex<Feed>>>, Json(req): Json<Value>) -> Json<Value> {
        let mut state = state.lock().unwrap();
        let result = match req["method"].as_str().unwrap() {
            "session.join" => {
                let cursor = req["params"]["from_seq"].as_u64();
                state.joins.push(cursor);
                let rows: Vec<_> = state
                    .rows
                    .iter()
                    .filter(|r| cursor.is_none_or(|n| r["seq"].as_u64().unwrap() > n))
                    .cloned()
                    .collect();
                state.from = rows
                    .last()
                    .and_then(|r| r["seq"].as_u64())
                    .or(cursor)
                    .map_or(0, |n| n + 1);
                json!({"rows": rows})
            }
            "session.ticket" => {
                state.tickets += 1;
                json!({"ticket": state.tickets.to_string()})
            }
            _ => panic!("unexpected request"),
        };
        Json(json!({"result": result}))
    }

    async fn ws(
        State(state): State<Arc<Mutex<Feed>>>,
        upgrade: axum::extract::ws::WebSocketUpgrade,
    ) -> impl axum::response::IntoResponse {
        upgrade.on_upgrade(move |mut socket| async move {
            let rows = {
                let mut state = state.lock().unwrap();
                state.sockets += 1;
                if state.sockets == 1 {
                    // Appended after the snapshot, this must arrive over the feed.
                    state
                        .rows
                        .push(json!({"seq": 1, "kind":"coin-toss.outcome"}));
                }
                state
                    .rows
                    .iter()
                    .filter(|r| r["seq"].as_u64().unwrap() >= state.from)
                    .cloned()
                    .collect::<Vec<_>>()
            };
            for row in rows {
                socket
                    .send(axum::extract::ws::Message::Text(row.to_string().into()))
                    .await
                    .unwrap();
            }
            socket
                .send(axum::extract::ws::Message::Close(None))
                .await
                .unwrap();
        })
    }

    #[test]
    fn client_rejoins_after_drain_with_last_received_cursor_and_fresh_ticket() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let state = Arc::new(Mutex::new(Feed {
            rows: vec![json!({"seq": 0, "kind":"coin-toss.outcome"})],
            ..Default::default()
        }));
        let listener = runtime
            .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
            .unwrap();
        let addr = listener.local_addr().unwrap();
        let router = Router::new()
            .route("/rpc", post(request))
            .route("/ws", axum::routing::get(ws))
            .with_state(state.clone());
        runtime.spawn(async move { axum::serve(listener, router).await.unwrap() });
        let client = Client::new(addr).unwrap();
        let recv = || {
            client
                .rows
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(5))
                .unwrap()["seq"]
                .as_u64()
                .unwrap()
        };
        assert_eq!(recv(), 0); // snapshot
        assert_eq!(recv(), 1); // first feed
        // Wait until the first empty rejoin has completed, then append later rows.
        let deadline = Instant::now() + Duration::from_secs(5);
        while state.lock().unwrap().sockets < 2 {
            assert!(
                Instant::now() < deadline,
                "client did not rejoin after drain"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        state
            .lock()
            .unwrap()
            .rows
            .extend([json!({"seq": 3}), json!({"seq": 4})]);
        assert_eq!(recv(), 3);
        assert_eq!(recv(), 4);
        let deadline = Instant::now() + Duration::from_secs(5);
        while !state.lock().unwrap().joins.contains(&Some(4)) {
            assert!(Instant::now() < deadline, "client did not advance cursor");
            std::thread::sleep(Duration::from_millis(10));
        }
        let feed = state.lock().unwrap();
        assert_eq!(feed.joins[0], None);
        assert_eq!(feed.joins[1], Some(1));
        assert!(feed.joins.contains(&Some(4)));
        assert!(feed.tickets >= 3);
        assert!(
            client.rows.lock().unwrap().try_recv().is_err(),
            "no duplicate rows"
        );
    }
}
