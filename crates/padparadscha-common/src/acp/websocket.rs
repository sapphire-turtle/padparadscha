//! WebSocket framing and the narrow application-wire adaptation of the pinned SDK.
use super::{Connection, MCP_SERVER, mcp_meta};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;
use tokio_tungstenite::tungstenite::Message;

/// RULING (plan 3127/3320–3323): SDK 5.2's native declaration has serverId, not url.
/// Adapt at request assembly, not in the fake peer. Retain serverId so native MCP routing
/// and its live rmcp registration still work; url is the application's additional field.
fn outgoing(line: &str) -> std::io::Result<String> {
    let mut frame: Value = serde_json::from_str(line)?;
    let method = frame["method"].as_str().unwrap_or("response").to_owned();
    if method == "session/new"
        && let Some(servers) = frame["params"]["mcpServers"].as_array_mut()
    {
        for server in servers {
            if server["type"] == "acp" && server["name"] == MCP_SERVER {
                server["url"] = json!("ws://127.0.0.1:0/rpc");
            }
        }
    }
    Ok(frame.to_string())
}

pub async fn open(settings: &crate::Settings) -> anyhow::Result<Arc<Connection>> {
    let (socket, _) = tokio_tungstenite::connect_async(&settings.acp_url).await?;
    let (sink, stream) = socket.split();
    // SDK 5.2 rejects mismatched initialize responses before the callback can inspect them.
    // Measure the raw response here and retain the exact application error for the caller.
    let mismatch = Arc::new(Mutex::new(None));
    let observed = Arc::clone(&mismatch);
    let sink = sink
        .sink_map_err(std::io::Error::other)
        .with(|line: String| async move { outgoing(&line).map(|line| Message::Text(line.into())) });
    let stream = stream.filter_map(move |frame| {
        let observed = Arc::clone(&observed);
        async move {
            let text = match frame {
                Ok(Message::Text(text)) => text.to_string(),
                Ok(Message::Binary(bytes)) => match String::from_utf8(bytes.to_vec()) {
                    Ok(text) => text,
                    Err(e) => return Some(Err(std::io::Error::other(e))),
                },
                Ok(_) => return None,
                Err(e) => return Some(Err(std::io::Error::other(e))),
            };
            let checked = (|| -> anyhow::Result<()> {
                let frame: Value = serde_json::from_str(&text)?;
                if let Some(version) = frame["result"].get("protocolVersion") {
                    let version = version
                        .as_str()
                        .map_or_else(|| version.to_string(), str::to_owned);
                    if let Err(error) = crate::check_api_version(&version) {
                        *observed.lock().unwrap() = Some(error);
                    }
                }
                // Content-block metadata is where event_message retains actual row identity.
                if let Some(content) = frame["params"]["update"].get("content")
                    && let Some(meta) = content.get("_meta")
                    && meta.get("seq").is_some()
                {
                    mcp_meta(meta)?;
                }
                // Preserve the received frame and validate application envelopes when present.
                // The fake (and older ACP peers) need not generate this application extension.
                for meta in [frame["params"].get("_meta"), frame["result"].get("_meta")]
                    .into_iter()
                    .flatten()
                {
                    if meta.get("seq").is_some() {
                        mcp_meta(meta)?;
                    }
                }
                Ok(())
            })();
            Some(checked.map(|()| text).map_err(std::io::Error::other))
        }
    });
    let (ready, receive) = oneshot::channel();
    tokio::spawn(async move {
        if let Err(error) = super::run_inner(
            agent_client_protocol::Lines::new(sink, stream),
            MCP_SERVER,
            env!("CARGO_PKG_VERSION"),
            true,
            ready,
        )
        .await
        {
            tracing::debug!(%error,"ACP connection closed");
        }
    });
    let result = receive.await?;
    if let Some(error) = mismatch.lock().unwrap().take() {
        return Err(error.into());
    }
    Ok(Arc::new(result?))
}
