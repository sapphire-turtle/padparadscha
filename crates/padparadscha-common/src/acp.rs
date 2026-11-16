//! The game server is the ACP client. One connection owns one active v1 session.
//!
//! `run` keeps the SDK's connection driver alive for the whole session. The returned handle
//! serializes prompts and collects the agent's replies; dropping all handles closes the driver.
use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::v1::schema::{
    ContentBlock, Implementation, InitializeRequest, TextContent,
};
use agent_client_protocol::{Client, ConnectTo};
use tokio::sync::{mpsc, oneshot};

struct Prompt {
    text: String,
    args: serde_json::Value,
    reply: oneshot::Sender<agent_client_protocol::Result<String>>,
}

/// A handle to the only session established by one `run` invocation.
#[derive(Clone)]
pub struct Connection {
    pub id: String,
    prompts: mpsc::UnboundedSender<Prompt>,
}

impl Connection {
    /// Send a user utterance or a session-language game line, then collect its reply.
    pub async fn user_message(
        &self,
        text: &str,
        args: &serde_json::Value,
    ) -> anyhow::Result<String> {
        let (reply, receive) = oneshot::channel();
        self.prompts
            .send(Prompt {
                text: text.to_owned(),
                args: args.clone(),
                reply,
            })
            .map_err(|_| anyhow::anyhow!("ACP session closed"))?;
        Ok(receive.await??)
    }
}

/// Drive an ACP connection and its one session until the last handle is dropped.
/// `ready` gets either the established session handle or the error opening it.
pub async fn run(
    transport: impl ConnectTo<Client> + 'static,
    name: &str,
    version: &str,
    ready: oneshot::Sender<agent_client_protocol::Result<Connection>>,
) -> agent_client_protocol::Result<()> {
    let (prompts, mut receiver) = mpsc::unbounded_channel::<Prompt>();
    let name = name.to_owned();
    let version = version.to_owned();
    let mut ready = Some(ready);
    let result = Client
        .v1()
        .name(&name)
        .connect_with(transport, async |cx| {
            let initialized = cx
                .send_request(
                    InitializeRequest::new(ProtocolVersion::V1)
                        .client_info(Implementation::new(&name, &version)),
                )
                .block_task()
                .await?;
            crate::check_api_version(&initialized.protocol_version.to_string())
                .map_err(agent_client_protocol::Error::into_internal_error)?;
            let mut session = cx
                .build_session_cwd()?
                .block_task()
                .start_session()
                .await?
                .session;
            let connection = Connection {
                id: session.session_id().to_string(),
                prompts,
            };
            if ready
                .take()
                .expect("ready once")
                .send(Ok(connection))
                .is_err()
            {
                return Ok(());
            }
            while let Some(prompt) = receiver.recv().await {
                let block = ContentBlock::Text(
                    TextContent::new(prompt.text).meta(prompt.args.as_object().cloned()),
                );
                let result = match session.send_prompt_blocks(vec![block]) {
                    Ok(()) => session.read_to_string().await,
                    Err(error) => Err(error),
                };
                let _ = prompt.reply.send(result);
            }
            Ok(())
        })
        .await;
    if let Some(ready) = ready {
        let _ = ready.send(result.clone().map(|()| unreachable!()));
    }
    result
}

/// Connect the configured websocket to the ACP server and open the one session.
#[cfg(not(target_arch = "wasm32"))]
pub async fn open(
    settings: &crate::Settings,
    name: &str,
    version: &str,
) -> anyhow::Result<Connection> {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message;
    let (socket, _) = tokio_tungstenite::connect_async(&settings.acp_url).await?;
    let (sink, stream) = socket.split();
    let sink = sink
        .sink_map_err(std::io::Error::other)
        .with(|line: String| async move { Ok(Message::Text(line.into())) });
    let stream = stream.filter_map(|frame| async move {
        match frame {
            Ok(Message::Text(text)) => Some(Ok(text.to_string())),
            Ok(Message::Binary(bytes)) => {
                Some(String::from_utf8(bytes.to_vec()).map_err(std::io::Error::other))
            }
            Ok(Message::Close(_)) => None,
            Ok(_) => None,
            Err(error) => Some(Err(std::io::Error::other(error))),
        }
    });
    let (ready, receive) = oneshot::channel();
    let name = name.to_owned();
    let version = version.to_owned();
    tokio::spawn(async move {
        if let Err(error) = run(
            agent_client_protocol::Lines::new(sink, stream),
            &name,
            &version,
            ready,
        )
        .await
        {
            tracing::warn!(%error, "ACP connection closed");
        }
    });
    Ok(receive.await??)
}
