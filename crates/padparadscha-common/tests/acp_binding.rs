#![cfg(not(target_arch = "wasm32"))]
use agent_client_protocol::{Channel, TransportFrame};
use futures_util::StreamExt;
use serde_json::{Value, json};

async fn receive(channel: &mut Channel) -> Value {
    let TransportFrame::Single(message) = channel.rx.next().await.unwrap() else {
        panic!("single frame")
    };
    serde_json::to_value(message).unwrap()
}
fn send(channel: &Channel, message: Value) {
    channel
        .tx
        .unbounded_send(TransportFrame::Single(
            serde_json::from_value(message).unwrap(),
        ))
        .unwrap();
}

#[tokio::test]
async fn client_initializes_one_session_and_sends_user_prompt() {
    let (transport, mut peer) = Channel::duplex();
    let (ready, receive_ready) = tokio::sync::oneshot::channel();
    let driver = tokio::spawn(padparadscha_common::acp::run(transport, "game", "1", ready));
    let init = receive(&mut peer).await;
    assert_eq!(init["method"], "initialize");
    send(
        &peer,
        json!({"jsonrpc":"2.0","id":init["id"],"result":{"protocolVersion":1}}),
    );
    let setup = receive(&mut peer).await;
    assert_eq!(setup["method"], "session/new");
    send(
        &peer,
        json!({"jsonrpc":"2.0","id":setup["id"],"result":{"sessionId":"one"}}),
    );
    let conn = receive_ready.await.unwrap().unwrap();
    assert_eq!(conn.id, "one");
    let prompt = tokio::spawn(async move {
        conn.user_message("hello", &json!({"outcome":"heads"}))
            .await
    });
    let request = receive(&mut peer).await;
    assert_eq!(request["method"], "session/prompt");
    assert_eq!(request["params"]["sessionId"], "one");
    assert_eq!(request["params"]["prompt"][0]["text"], "hello");
    assert_eq!(request["params"]["prompt"][0]["_meta"]["outcome"], "heads");
    send(
        &peer,
        json!({"jsonrpc":"2.0","id":request["id"],"result":{"stopReason":"end_turn"}}),
    );
    assert_eq!(prompt.await.unwrap().unwrap(), "");
    driver.await.unwrap().unwrap();
}
