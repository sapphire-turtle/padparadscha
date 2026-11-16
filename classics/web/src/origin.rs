//! The page's own origin (§8, §9): fetch carries the token; the socket carries only a ticket.

use serde_json::Value;
use std::sync::OnceLock;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Request, RequestInit, RequestMode, Response, WebSocket};

static TOKEN: OnceLock<String> = OnceLock::new();

/// Take the page's token from its query, decoding form escaping exactly once.
pub fn set_token_from_query(query: &str) {
    if let Some((_, token)) = url::form_urlencoded::parse(query.trim_start_matches('?').as_bytes())
        .find(|(name, _)| name == "token")
    {
        let _ = TOKEN.set(token.into_owned());
    }
}

/// One origin, without a path, query or fragment.
pub struct Origin(pub String);

impl Origin {
    /// The origin the browser loaded this page from, including pages in a site's corner.
    pub fn same_origin() -> Self {
        let location = web_sys::window().expect("a browser").location();
        let origin = location
            .origin()
            .expect("this page was loaded from an origin");
        Self(
            origin
                .trim_end_matches('/')
                .split(['?', '#'])
                .next()
                .unwrap()
                .to_string(),
        )
    }

    /// JSON-RPC 2.0 over fetch. A failed binding is one line and never poisons the next attempt.
    pub async fn rpc(&self, method: &str, params: Value) -> anyhow::Result<Value> {
        let unavailable = |_: JsValue| anyhow::anyhow!("server not found");
        let init = RequestInit::new();
        init.set_method("POST");
        init.set_mode(RequestMode::SameOrigin);
        init.set_body(&JsValue::from_str(
            &serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "method": method, "params": params,
            })
            .to_string(),
        ));
        let request = Request::new_with_str_and_init(&format!("{}/rpc", self.0), &init)
            .map_err(unavailable)?;
        request
            .headers()
            .set("content-type", "application/json")
            .map_err(unavailable)?;
        if let Some(token) = TOKEN.get() {
            request
                .headers()
                .set("authorization", &format!("Bearer {token}"))
                .map_err(unavailable)?;
        }
        let window = web_sys::window().ok_or_else(|| anyhow::anyhow!("server not found"))?;
        let response: Response =
            wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&request))
                .await
                .map_err(unavailable)?
                .dyn_into()
                .map_err(unavailable)?;
        let text = wasm_bindgen_futures::JsFuture::from(response.text().map_err(unavailable)?)
            .await
            .map_err(unavailable)?;
        let answered: Value = serde_json::from_str(text.as_string().as_deref().unwrap_or_default())
            .map_err(|_| anyhow::anyhow!("server not found"))?;
        if let Some(error) = answered.get("error") {
            anyhow::bail!("{}", error["message"].as_str().unwrap_or("refused"));
        }
        answered
            .get("result")
            .filter(|_| response.ok())
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("server not found"))
    }

    /// The browser upgrades HTTP(S) to WS(S); no token or other credential is in the query.
    pub fn ws(&self, ticket: &str) -> WebSocket {
        let ticket: String = url::form_urlencoded::byte_serialize(ticket.as_bytes()).collect();
        WebSocket::new(&format!("{}/ws?ticket={ticket}", self.0)).expect("the origin's own feed")
    }
}
