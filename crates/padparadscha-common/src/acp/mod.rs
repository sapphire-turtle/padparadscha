//! ACP v1. The callback owns the active session until the last handle is dropped.
use agent_client_protocol::mcp_server::McpServer;
use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::v1::schema::{
    ContentBlock, Implementation, InitializeRequest, TextContent,
};
use agent_client_protocol::{Client, ConnectTo};
use agent_client_protocol_rmcp::rmcp_3::McpServerExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

#[cfg(not(target_arch = "wasm32"))]
mod websocket;
#[cfg(not(target_arch = "wasm32"))]
pub use websocket::open;

pub const MCP_SERVER: &str = "table";
pub const TOOLS: &[&str] = &["toss_coin", "log"];

/// The row's four fields, independent of either reader's display language.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConnMeta {
    pub seq: crate::Seq,
    pub kind: String,
    pub args: serde_json::Value,
    pub who: crate::Actor,
}

pub fn conn_meta(event: &crate::Event) -> serde_json::Value {
    serde_json::json!({"seq":event.seq,"kind":event.kind,"args":event.args,"who":event.args["who"]})
}

/// Recover a known game row without leaking untrusted strings to obtain a static Fluent id.
pub fn mcp_meta(meta: &serde_json::Value) -> anyhow::Result<crate::Event> {
    let ConnMeta {
        seq,
        kind,
        args,
        who,
    } = serde_json::from_value(meta.clone())?;
    anyhow::ensure!(
        args["who"] == serde_json::to_value(who)?,
        "metadata actor disagrees with row args"
    );
    let kind = match kind.as_str() {
        "coin-toss.outcome" => "coin-toss.outcome",
        "table.note" => "table.note",
        _ => anyhow::bail!("unknown event kind {kind}"),
    };
    Ok(crate::Event::of(seq, kind, args))
}

struct Prompt {
    text: String,
    args: serde_json::Value,
    meta: Option<serde_json::Map<String, serde_json::Value>>,
    reply: oneshot::Sender<agent_client_protocol::Result<String>>,
}

#[derive(Clone, Debug)]
pub struct Connection {
    pub id: String,
    prompts: mpsc::UnboundedSender<Prompt>,
}

impl Connection {
    pub async fn user_message(&self, text: &str, args: &serde_json::Value) -> anyhow::Result<()> {
        self.user_message_reply(text, args).await?;
        Ok(())
    }

    /// The Task 9 reply-collecting interface; the Task 13 interface discards only the reply text.
    pub async fn user_message_reply(
        &self,
        text: &str,
        args: &serde_json::Value,
    ) -> anyhow::Result<String> {
        self.prompt(text, args, None).await
    }

    /// Send an actual log row, retaining its own sequence, kind, arguments and actor.
    /// Plain utterances have no row identity and must not manufacture one.
    pub async fn event_message(&self, text: &str, row: &crate::Event) -> anyhow::Result<String> {
        self.prompt(text, &row.args, conn_meta(row).as_object().cloned())
            .await
    }

    async fn prompt(
        &self,
        text: &str,
        args: &serde_json::Value,
        meta: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> anyhow::Result<String> {
        let (reply, receive) = oneshot::channel();
        self.prompts
            .send(Prompt {
                text: text.into(),
                args: args.clone(),
                meta,
                reply,
            })
            .map_err(|_| anyhow::anyhow!("ACP session closed"))?;
        Ok(receive.await??)
    }
}

struct TossTools;
impl rmcp::ServerHandler for TossTools {
    fn get_info(&self) -> rmcp::model::ServerConfig {
        rmcp::model::ServerConfig::new(
            rmcp::model::ServerCapabilities::builder()
                .enable_tools()
                .build(),
        )
    }

    async fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<rmcp::model::ListToolsResult, rmcp::ErrorData> {
        let schema = Arc::new(serde_json::Map::from_iter([(
            String::from("type"),
            serde_json::json!("object"),
        )]));
        Ok(rmcp::model::ListToolsResult::with_all_items(
            TOOLS
                .iter()
                .map(|name| rmcp::model::Tool::new(*name, *name, Arc::clone(&schema)))
                .collect(),
        ))
    }
}

/// Compatibility driver used by Task 9's channel transport test and the real stdio peer.
pub async fn run(
    transport: impl ConnectTo<Client> + 'static,
    name: &str,
    version: &str,
    ready: oneshot::Sender<agent_client_protocol::Result<Connection>>,
) -> agent_client_protocol::Result<()> {
    run_inner(transport, name, version, false, ready).await
}

async fn run_inner(
    transport: impl ConnectTo<Client> + 'static,
    name: &str,
    version: &str,
    tools: bool,
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
            let mut session = if tools {
                cx.build_session_cwd()?
                    .with_mcp_server(McpServer::from_rmcp(MCP_SERVER, || TossTools))?
                    .block_task()
                    .start_session()
                    .await?
                    .session
            } else {
                cx.build_session_cwd()?
                    .block_task()
                    .start_session()
                    .await?
                    .session
            };
            let connection = Connection {
                id: session.session_id().to_string(),
                prompts,
            };
            // Move (do not clone) the sole sender out of the driver: retaining it prevents shutdown.
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
                    TextContent::new(prompt.text)
                        .meta(prompt.meta.or_else(|| prompt.args.as_object().cloned())),
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
