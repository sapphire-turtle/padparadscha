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
            let result: anyhow::Result<()> = async {
                let joined = rpc(addr, "session.join", json!({"from_seq": null})).await?;
                for row in joined["rows"].as_array().into_iter().flatten() {
                    tx.send(row.clone())?;
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
                        tx.send(serde_json::from_str(frame.to_text()?)?)?;
                    }
                }
                Ok(())
            }
            .await;
            if let Err(error) = result {
                tracing::warn!(%error, "chat connection closed");
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
