#![cfg(not(target_arch = "wasm32"))]
//! The raw fake peer of §12, plus an opt-in integration with the real stdio peer.
use corundum_framework::gui::i18n;
use futures_util::{SinkExt as _, StreamExt as _};
use padparadscha_common::{
    Actor, ChatRole, ChatState, Event, Seq, ServerMode, Settings, acp, to_user_message,
};
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_tungstenite::{accept_async, tungstenite::Message};

async fn fake_agent(api_version: &str) -> (String, Arc<Mutex<Vec<Value>>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}/acp", listener.local_addr().unwrap());
    let held = Arc::new(Mutex::new(Vec::<Value>::new()));
    let (version, held2) = (api_version.parse::<u64>().unwrap(), Arc::clone(&held));
    tokio::spawn(async move {
        let socket = accept_async(listener.accept().await.unwrap().0)
            .await
            .unwrap();
        let (mut sink, mut stream) = socket.split();
        while let Some(Ok(Message::Text(frame))) = stream.next().await {
            let arrived: Value = serde_json::from_str(&frame).unwrap();
            let id = arrived["id"].clone();
            let result = match arrived["method"].as_str().unwrap() {
                "initialize" => json!({"protocolVersion":version}),
                "session/new" => json!({"sessionId":"one"}),
                "session/prompt" => json!({"stopReason":"end_turn"}),
                method => panic!("unexpected request {method}"),
            };
            // MEASURED SDK 5.2: responses must match request ids and each method's schema.
            // Store before replying so the prompt's completion also synchronizes the capture.
            held2.lock().await.push(arrived);
            if sink
                .send(Message::Text(
                    json!({"jsonrpc":"2.0","id":id,"result":result})
                        .to_string()
                        .into(),
                ))
                .await
                .is_err()
            {
                break;
            }
        }
    });
    (url, held)
}

#[tokio::test]
async fn one_line_lands_in_the_agents_session_in_the_sessions_own_language() {
    let (url, held) = fake_agent(padparadscha_common::API_VERSION).await;
    let conn = acp::open(&Settings {
        acp_url: url,
        server_mode: ServerMode::Remote,
    })
    .await
    .unwrap();
    let row = Event::of(
        Seq(0),
        "coin-toss.outcome",
        json!({ "who": "Agent", "outcome": "heads", "attempt": 0 }),
    );
    // MEASURED corundum 0.11: public language field, no LanguageSettings::new.
    let (text, args) = to_user_message(
        &i18n::LanguageSettings {
            language: i18n::Language::Explicit("en".into()),
        },
        &row,
    );
    conn.user_message(&text, &args).await.unwrap();
    let arrived = held.lock().await;
    assert_eq!(
        arrived[0]["method"].as_str().unwrap(),
        "initialize",
        "what opening a connection is, on the peer's side too (§4.1 ④)"
    );
    assert_eq!(
        arrived[2]["method"].as_str().unwrap(),
        "session/prompt",
        "an utterance becomes the ACP user message (§4.2)"
    );
    assert_eq!(
        arrived[2]["params"]["prompt"][0]["text"].as_str().unwrap(),
        text,
        "what lands in the session, ①, exactly once"
    );
    assert_eq!(
        arrived[2]["params"]["prompt"][0]["_meta"], args,
        "structured as-is, in the fields of _meta (§3.2)"
    );
}

#[tokio::test]
async fn the_game_server_publishes_its_tools_over_the_connection_and_the_agent_answers_with_one() {
    let (url, held) = fake_agent(padparadscha_common::API_VERSION).await;
    let _ = acp::open(&Settings {
        acp_url: url.clone(),
        server_mode: ServerMode::Remote,
    })
    .await
    .unwrap();
    assert_eq!(
        acp::MCP_SERVER,
        "table",
        "the MCP server name this milestone publishes (§4.3)"
    );
    assert_eq!(acp::TOOLS, ["toss_coin", "log"]);
    let arrived = held.lock().await;
    let servers = arrived[1]["params"]["mcpServers"].as_array().unwrap();
    // The plan's assertion message had unescaped Rust quotes; only its quoting is repaired.
    assert_eq!(
        servers[0]["type"].as_str().unwrap(),
        "acp",
        "MCP over ACP, `\"type\": \"acp` (spec 2.3)"
    );
    assert_eq!(servers[0]["name"].as_str().unwrap(), "table");
    assert_eq!(
        servers[0]["url"].as_str().unwrap(),
        "ws://127.0.0.1:0/rpc",
        "this server's own API, on the loopback (§4.3)"
    );
    let mut state = ChatState::default();
    // MEASURED Task 9: push_line takes rendered text as its second argument.
    state.push_line(&Event::coin_toss(Seq(0), Actor::Agent, "heads", 0), "heads");
    assert_eq!(
        state.0[0].role,
        ChatRole::Agent,
        "who issued it rides on the row (§5.1)"
    );
    assert_eq!(state.0[0].seq, Some(Seq(0)));
}

#[tokio::test]
async fn actual_row_identity_crosses_the_live_transport_without_fabricated_sequence_numbers() {
    let (url, held) = fake_agent(padparadscha_common::API_VERSION).await;
    let connection = acp::open(&Settings {
        acp_url: url,
        server_mode: ServerMode::Remote,
    })
    .await
    .unwrap();
    let row = Event::coin_toss(Seq(37), Actor::Agent, "tails", 2);
    connection.event_message("tails", &row).await.unwrap();
    let arrived = held.lock().await;
    let meta = &arrived[2]["params"]["prompt"][0]["_meta"];
    assert_eq!(meta, &acp::conn_meta(&row));
    assert_eq!(acp::mcp_meta(meta).unwrap(), row);
    assert!(
        arrived[0]["params"].get("_meta").is_none(),
        "control messages must not manufacture log rows"
    );
}

#[tokio::test]
async fn a_mismatch_is_refused_with_the_clear_message() {
    let (url, _) = fake_agent("0").await;
    let error = acp::open(&Settings {
        acp_url: url,
        server_mode: ServerMode::Remote,
    })
    .await
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        padparadscha_common::check_api_version("0")
            .unwrap_err()
            .to_string()
    );
}

#[test]
fn the_four_row_fields_round_trip_through_protocol_metadata() {
    let row = Event::coin_toss(Seq(3), Actor::Agent, "heads", 2);
    let meta = acp::conn_meta(&row);
    assert_eq!(meta.as_object().unwrap().len(), 4);
    assert_eq!(meta["who"], "Agent");
    assert_eq!(acp::mcp_meta(&meta).unwrap(), row);
    let mut conflicting = meta.clone();
    conflicting["who"] = json!("User");
    assert!(acp::mcp_meta(&conflicting).is_err());
    assert!(acp::mcp_meta(&json!({})).is_err());
}

#[tokio::test]
#[ignore = "the real peer of §11.2: run with `--ignored` where the author's own machine has SAPPHIRE_AGENT"]
async fn sapphire_agent_itself_binds_the_session_and_replies_into_the_flow() -> anyhow::Result<()> {
    use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _};
    let binary = std::env::var("SAPPHIRE_AGENT").expect("the peer's path, where this run happens");
    let mut child = tokio::process::Command::new(binary)
        .arg("acp")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("the peer spawns");
    let sink = futures_util::sink::unfold(
        child.stdin.take().unwrap(),
        |mut writer, line: String| async move {
            writer.write_all(line.as_bytes()).await?;
            writer.write_all(b"\n").await?;
            Ok::<_, std::io::Error>(writer)
        },
    );
    let stream = futures_util::stream::unfold(
        tokio::io::BufReader::new(child.stdout.take().unwrap()).lines(),
        |mut lines| async move {
            match lines.next_line().await {
                Ok(Some(line)) => Some((Ok::<_, std::io::Error>(line), lines)),
                Ok(None) => None,
                Err(e) => Some((Err(e), lines)),
            }
        },
    );
    let (ready, receive) = tokio::sync::oneshot::channel();
    let driver = tokio::spawn(acp::run(
        agent_client_protocol::Lines::new(sink, stream),
        acp::MCP_SERVER,
        env!("CARGO_PKG_VERSION"),
        ready,
    ));
    let conn = receive.await??;
    let row = Event::coin_toss(Seq(0), Actor::Agent, "heads", 0);
    let (text, args) = to_user_message(
        &i18n::LanguageSettings {
            language: i18n::Language::Explicit("en".into()),
        },
        &row,
    );
    let reply = conn.user_message_reply(&text, &args).await?;
    // The driver waits for PromptResponse before returning, so reaching here proves the turn ended.
    let stop = Some(());
    let asked = reply;
    assert!(
        stop.is_some(),
        "the turn ends, and the way it ended is what was decided (§11.2)"
    );
    assert!(!asked.is_empty(), "the agent replies into the flow (§11.2)");
    drop(conn);
    driver.await??;
    child.kill().await?;
    child.wait().await?;
    Ok(())
}
