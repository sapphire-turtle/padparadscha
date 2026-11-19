#![cfg(not(target_arch = "wasm32"))]
//! §12 and §11.1: the actual binary, two clients, exactly one rendered conversation line.
use futures_util::StreamExt as _;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt as _, AsyncReadExt as _, AsyncWriteExt as _, BufReader};
use tokio::process::{Child, Command};
use tokio_tungstenite::connect_async;

async fn headless() -> (std::net::SocketAddr, Child) {
    // MEASURED: stdout is not piped by default; retain a kill-on-drop child on every test path.
    let mut child = Command::new(env!("CARGO_BIN_EXE_padparadscha-classics-headless"))
        .args(["--bind", "127.0.0.1:0"])
        .stdout(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut line = String::new();
    tokio::time::timeout(
        std::time::Duration::from_secs(10),
        BufReader::new(child.stdout.take().unwrap()).read_line(&mut line),
    )
    .await
    .unwrap()
    .unwrap();
    let printed: Value = serde_json::from_str(&line).unwrap();
    (
        printed["fields"]["addr"].as_str().unwrap().parse().unwrap(),
        child,
    )
}

async fn rpc(addr: std::net::SocketAddr, id: u64, method: &str, params: Value) -> Value {
    let body = json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}).to_string();
    // MEASURED hyper/axum: HTTP/1.1 Host, JSON Content-Type and close delimit a whole response.
    let request = format!(
        "POST /rpc HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    let mut socket = tokio::net::TcpStream::connect(addr).await.unwrap();
    socket.write_all(request.as_bytes()).await.unwrap();
    let mut answered = String::new();
    socket.read_to_string(&mut answered).await.unwrap();
    let (_, body) = answered.split_once("\r\n\r\n").unwrap();
    let value: Value = serde_json::from_str(body).unwrap();
    assert!(value.get("error").is_none(), "{value}");
    value["result"].clone()
}

#[tokio::test]
async fn one_command_leaves_one_line_and_the_client_that_joins_after_gets_the_same_one() {
    let (addr, mut child) = headless().await;
    let first = rpc(addr, 1, "session.join", json!({"from_seq":null})).await;
    assert!(
        first["rows"].as_array().unwrap().is_empty(),
        "a new session's log is empty"
    );
    let command = rpc(addr, 2, "game.command", json!({"who":"User","kind":"toss"})).await;
    let rows = rpc(addr, 3, "session.join", json!({"from_seq":null})).await["rows"]
        .as_array()
        .unwrap()
        .clone();
    // MEASURED landed Seq is numeric. Task 13 ruling: repair only the disagreeing index site.
    assert_eq!(
        rows[0]["seq"], command["seq"],
        "one command, exactly one row, one seq (§5.1)"
    );
    // MEASURED egui is re-exported by bevy_egui, not common; corundum needs its bundle installed.
    let ctx = bevy_egui::egui::Context::default();
    corundum_framework::gui::i18n::register(&ctx, padparadscha_common::TRANSLATIONS);
    corundum_framework::gui::i18n::install(
        &ctx,
        &corundum_framework::gui::i18n::LanguageSettings {
            language: corundum_framework::gui::i18n::Language::Explicit("en".into()),
        },
    );
    // MEASURED Event kind is static; a bounded test-only JSON fixture supplies that lifetime.
    let event: padparadscha_common::Event =
        serde_json::from_str(Box::leak(rows[0].to_string().into_boxed_str())).unwrap();
    let line = padparadscha_common::line_of(&ctx, &event);
    assert!(
        line.contains("heads") || line.contains("tails") || line.contains("edge"),
        "one line, one adjudication (§11.1)"
    );
    let later = rpc(addr, 4, "session.join", json!({"from_seq":event.seq.0})).await;
    assert!(
        later["rows"].as_array().unwrap().is_empty(),
        "nothing arrived since, so nothing is missing (§7.3)"
    );
    let ticket = rpc(addr, 5, "session.ticket", json!({})).await["ticket"]
        .as_str()
        .unwrap()
        .to_owned();
    let (socket, _) = connect_async(format!("ws://{addr}/ws?ticket={ticket}"))
        .await
        .unwrap();
    // MEASURED (tungstenite 0.30): the raw reader carries the close handshake as items -- the Close
    // control frame on every runner, and on windows-latest also the post-close protocol error
    // it resolves as an item. The feed is application rows, and application rows alone: data frames.
    // Everything the close handshake leaves in the stream is framing, not content.
    let socket = socket.filter_map(|frame| async {
        match frame {
            Ok(m) if m.is_text() || m.is_binary() => Some(Ok::<_, ()>(m)),
            _ => None,
        }
    });
    tokio::pin!(socket);
    assert!(
        socket.next().await.is_none(),
        "the feed starts at the seq the snapshot ended at, and there is nothing after it"
    );
    child.kill().await.unwrap();
    child.wait().await.unwrap();
}

#[tokio::test]
async fn a_queued_command_does_not_block_and_the_order_stays_the_orders() {
    let (addr, mut child) = headless().await;
    let joined = rpc(addr, 1, "session.join", json!({"from_seq":null})).await;
    assert!(
        joined["seed"].is_number(),
        "the one seed of the session, handed at join (§7.1)"
    );
    let toss = rpc(
        addr,
        2,
        "game.command",
        json!({"who":"Agent","kind":"toss"}),
    )
    .await;
    let note = rpc(
        addr,
        3,
        "game.command",
        json!({"who":"User","kind":"log","note":"the one that waited"}),
    )
    .await;
    let rows = rpc(addr, 4, "session.join", json!({"from_seq":null})).await["rows"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(
        rows.len(),
        2,
        "the second command never waited on the first's outcome (§10)"
    );
    assert_eq!(
        rows[0]["seq"], toss["seq"],
        "in the order the queue accepted them"
    );
    assert_eq!(
        rows[1]["seq"], note["seq"],
        "each carrying the one seq its command was tagged with"
    );
    child.kill().await.unwrap();
    child.wait().await.unwrap();
}
