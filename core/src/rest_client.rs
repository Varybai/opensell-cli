use crate::errors::{McpError, ToolError};
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use std::time::Duration;

const DEFAULT_BASE_URL: &str = "https://varybai.online/api";

pub struct RestClient {
    pub base_url: String,
    pub agent_token: String,
}

impl RestClient {
    pub fn new(base_url: Option<String>, token: Option<String>) -> Self {
        Self {
            base_url: base_url.unwrap_or_else(|| DEFAULT_BASE_URL.to_string()),
            agent_token: token.unwrap_or_default(),
        }
    }

    pub fn from_env() -> Self {
        let base = std::env::var("AIXIANYU_BASE_URL")
            .ok()
            .filter(|s| !s.is_empty());
        let token = std::env::var("AIXIANYU_AGENT_TOKEN").ok();
        Self::new(base, token)
    }

    fn client(&self, timeout_secs: u64) -> Client {
        let mut builder = Client::builder().timeout(Duration::from_secs(timeout_secs));
        if !self.agent_token.is_empty() {
            if let Ok(val) =
                reqwest::header::HeaderValue::from_str(&format!("Agent {}", self.agent_token))
            {
                let mut headers = reqwest::header::HeaderMap::new();
                headers.insert(reqwest::header::AUTHORIZATION, val);
                builder = builder.default_headers(headers);
            }
        }
        builder.build().expect("reqwest client")
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    /// Read status + headers + bytes once, then branch on success/error.
    /// This avoids the reqwest ownership issue where .json() consumes the response.
    ///
    /// `not_found` names the resource a 404 refers to (order / conversation /
    /// item) so the message is accurate; the exit code stays in the not-found
    /// family regardless.
    async fn send_and_parse(
        &self,
        req: reqwest::RequestBuilder,
        not_found: McpError,
    ) -> Result<Value, ToolError> {
        let resp = req.send().await.map_err(|_| ToolError::server_error())?;
        let status = resp.status();
        let headers = resp.headers().clone();
        let bytes = resp.bytes().await.map_err(|_| ToolError::server_error())?;

        if status.is_success() {
            serde_json::from_slice::<Value>(&bytes).map_err(|_| ToolError::server_error())
        } else {
            Err(map_error(status, &headers, &bytes, not_found))
        }
    }

    // ── GET endpoints ────────────────────────────────────────────────────────

    pub async fn ping(&self) -> Result<Value, ToolError> {
        let health = self
            .send_and_parse(self.client(10).get(self.url("/health")), McpError::ItemNotFound)
            .await?;
        Ok(json!({"status": "pong", "backend": self.base_url, "health": health}))
    }

    pub async fn search_items(&self, params: &Value) -> Result<Value, ToolError> {
        let query: Vec<(String, String)> = params
            .as_object()
            .map(|m| {
                m.iter()
                    .map(|(k, v)| (k.clone(), value_to_query(v)))
                    .collect()
            })
            .unwrap_or_default();
        self.send_and_parse(
            self.client(10).get(self.url("/v1/items")).query(&query),
            McpError::ItemNotFound,
        )
        .await
    }

    pub async fn get_item(&self, item_id: i64) -> Result<Value, ToolError> {
        self.send_and_parse(
            self.client(10).get(self.url(&format!("/v1/items/{item_id}"))),
            McpError::ItemNotFound,
        )
        .await
    }

    pub async fn list_categories(&self) -> Result<Value, ToolError> {
        self.send_and_parse(
            self.client(10).get(self.url("/v1/categories")),
            McpError::ItemNotFound,
        )
        .await
    }

    pub async fn get_order(&self, order_id: i64) -> Result<Value, ToolError> {
        self.send_and_parse(
            self.client(10).get(self.url(&format!("/v1/orders/{order_id}"))),
            McpError::OrderNotFound,
        )
        .await
    }

    pub async fn get_wallet(&self) -> Result<Value, ToolError> {
        self.send_and_parse(
            self.client(10).get(self.url("/v1/wallet")),
            McpError::ItemNotFound,
        )
        .await
    }

    pub async fn list_conversations(&self) -> Result<Value, ToolError> {
        self.send_and_parse(
            self.client(10).get(self.url("/v1/conversations")),
            McpError::ItemNotFound,
        )
        .await
    }

    pub async fn get_messages(
        &self,
        conversation_id: i64,
        params: &Value,
    ) -> Result<Value, ToolError> {
        let query: Vec<(String, String)> = params
            .as_object()
            .map(|m| {
                m.iter()
                    .map(|(k, v)| (k.clone(), value_to_query(v)))
                    .collect()
            })
            .unwrap_or_default();
        self.send_and_parse(
            self.client(10)
                .get(self.url(&format!("/v1/conversations/{conversation_id}/messages")))
                .query(&query),
            McpError::ConversationNotFound,
        )
        .await
    }

    pub async fn get_token_info(&self) -> Result<Value, ToolError> {
        self.send_and_parse(
            self.client(10).get(self.url("/v1/agent/token/me")),
            McpError::ItemNotFound,
        )
        .await
    }

    // ── POST / PUT / DELETE endpoints ────────────────────────────────────────

    pub async fn contact_seller(&self, item_id: i64, message: &str) -> Result<Value, ToolError> {
        self.send_and_parse(
            self.client(10)
                .post(self.url("/v1/conversations/contact"))
                .json(&json!({
                    "item_id": item_id,
                    "type": "text",
                    "body": {"text": message}
                })),
            McpError::ItemNotFound,
        )
        .await
    }

    pub async fn send_message(
        &self,
        conversation_id: i64,
        content: &str,
    ) -> Result<Value, ToolError> {
        self.send_and_parse(
            self.client(10)
                .post(self.url(&format!("/v1/conversations/{conversation_id}/messages")))
                .json(&json!({"type": "text", "body": {"text": content}})),
            McpError::ConversationNotFound,
        )
        .await
    }

    pub async fn publish_item(&self, payload: &Value) -> Result<Value, ToolError> {
        self.send_and_parse(
            self.client(10).post(self.url("/v1/items")).json(payload),
            McpError::ItemNotFound,
        )
        .await
    }

    pub async fn update_item(&self, item_id: i64, payload: &Value) -> Result<Value, ToolError> {
        self.send_and_parse(
            self.client(10)
                .put(self.url(&format!("/v1/items/{item_id}")))
                .json(payload),
            McpError::ItemNotFound,
        )
        .await
    }

    pub async fn delist_item(&self, item_id: i64) -> Result<Value, ToolError> {
        // DELETE returns 204 No Content on success; synthesize a stable result.
        let resp = self
            .client(10)
            .delete(self.url(&format!("/v1/items/{item_id}")))
            .send()
            .await
            .map_err(|_| ToolError::server_error())?;
        let status = resp.status();
        let headers = resp.headers().clone();
        let bytes = resp.bytes().await.map_err(|_| ToolError::server_error())?;
        if status.is_success() {
            Ok(json!({"item_id": item_id, "status": "delisted"}))
        } else {
            Err(map_error(status, &headers, &bytes, McpError::ItemNotFound))
        }
    }

    pub async fn upload_images(
        &self,
        files: Vec<(String, Vec<u8>, String)>,
    ) -> Result<Vec<String>, ToolError> {
        let mut form = reqwest::multipart::Form::new();
        for (name, data, ctype) in files {
            let part = reqwest::multipart::Part::bytes(data)
                .file_name(name)
                .mime_str(&ctype)
                .map_err(|_| ToolError::server_error())?;
            form = form.part("files", part);
        }
        let resp = self
            .client(60)
            .post(self.url("/v1/images"))
            .multipart(form)
            .send()
            .await
            .map_err(|_| ToolError::server_error())?;
        let status = resp.status();
        let headers = resp.headers().clone();
        let bytes = resp.bytes().await.map_err(|_| ToolError::server_error())?;
        if !status.is_success() {
            return Err(map_error(status, &headers, &bytes, McpError::ItemNotFound));
        }
        let body: Value = serde_json::from_slice(&bytes).map_err(|_| ToolError::server_error())?;
        Ok(body["images"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|img| img["id"].as_str().map(|s| s.to_string()))
            .collect())
    }

    pub async fn place_order(&self, item_id: i64, quantity: i64) -> Result<Value, ToolError> {
        self.send_and_parse(
            self.client(10)
                .post(self.url("/v1/orders"))
                .json(&json!({"item_id": item_id, "quantity": quantity})),
            McpError::ItemNotFound,
        )
        .await
    }

    pub async fn pay_order(&self, order_id: i64) -> Result<Value, ToolError> {
        self.send_and_parse(
            self.client(10)
                .post(self.url(&format!("/v1/orders/{order_id}/pay")))
                .json(&json!({"payment_method": "wallet"})),
            McpError::OrderNotFound,
        )
        .await
    }

    pub async fn wallet_withdraw(&self, amount: f64) -> Result<Value, ToolError> {
        self.send_and_parse(
            self.client(10)
                .post(self.url("/v1/wallet/withdraw"))
                .json(&json!({"amount": amount})),
            McpError::ItemNotFound,
        )
        .await
    }

    pub async fn reveal_credential(&self, order_id: i64) -> Result<Value, ToolError> {
        self.send_and_parse(
            self.client(10)
                .post(self.url(&format!("/v1/orders/{order_id}/reveal-credential"))),
            McpError::OrderNotFound,
        )
        .await
    }

    pub async fn confirm_order(&self, order_id: i64) -> Result<Value, ToolError> {
        self.send_and_parse(
            self.client(10)
                .post(self.url(&format!("/v1/orders/{order_id}/confirm"))),
            McpError::OrderNotFound,
        )
        .await
    }

    pub async fn deliver_credential(
        &self,
        order_id: i64,
        credential: &Value,
    ) -> Result<Value, ToolError> {
        self.send_and_parse(
            self.client(10)
                .post(self.url(&format!("/v1/orders/{order_id}/deliver")))
                .json(&json!({"credential": credential})),
            McpError::OrderNotFound,
        )
        .await
    }
}

// ── Error mapping ─────────────────────────────────────────────────────────────

fn map_error(
    status: StatusCode,
    headers: &reqwest::header::HeaderMap,
    bytes: &[u8],
    not_found: McpError,
) -> ToolError {
    match status.as_u16() {
        401 => ToolError::unauthorized(),
        403 => {
            let detail = serde_json::from_slice::<Value>(bytes)
                .ok()
                .and_then(|body| body.get("detail").cloned());
            if let Some(d) = detail.as_ref().filter(|d| d.is_object()) {
                let err_field = d.get("error").and_then(|e| e.as_str());
                if err_field == Some("insufficient_scope") {
                    let required = match d.get("required") {
                        Some(Value::Array(a)) => a
                            .iter()
                            .filter_map(|x| x.as_str())
                            .collect::<Vec<_>>()
                            .join(", "),
                        Some(Value::String(s)) => s.clone(),
                        Some(other) => other.to_string(),
                        None => String::new(),
                    };
                    let required = if required.is_empty() {
                        "unknown".to_string()
                    } else {
                        required
                    };
                    return ToolError::insufficient_scope(&required);
                }
                if err_field == Some("sandbox_limit") || err_field == Some("agent_sandbox_limit") {
                    let limit = d
                        .get("limit")
                        .map(|v| match v {
                            Value::String(s) => s.clone(),
                            other => other.to_string(),
                        })
                        .unwrap_or_else(|| "unknown".to_string());
                    return ToolError::agent_sandbox_limit(&limit);
                }
            }
            ToolError::forbidden(&forbidden_reason(detail.as_ref()))
        }
        404 => ToolError::new(not_found, not_found_message(not_found)),
        422 => {
            let details = serde_json::from_slice::<Value>(bytes)
                .ok()
                .and_then(|body| body.get("detail").cloned())
                .map(|d| match d {
                    Value::String(s) => s,
                    other => other.to_string(),
                })
                .unwrap_or_else(|| "validation error".to_string());
            ToolError::invalid_args(&details)
        }
        429 => {
            let retry = headers
                .get("retry-after")
                .or_else(|| headers.get("Retry-After"))
                .and_then(|v| v.to_str().ok())
                .unwrap_or("60")
                .to_string();
            ToolError::rate_limited(&retry)
        }
        _ => ToolError::server_error(),
    }
}

fn not_found_message(e: McpError) -> &'static str {
    match e {
        McpError::OrderNotFound => "Order not found",
        McpError::ConversationNotFound => "Conversation not found",
        _ => "Item not found or delisted",
    }
}

fn forbidden_reason(detail: Option<&Value>) -> String {
    match detail {
        Some(Value::String(s)) if !s.is_empty() => s.clone(),
        Some(Value::Object(_)) => detail
            .and_then(|d| {
                d.get("message")
                    .or_else(|| d.get("code"))
                    .or_else(|| d.get("error"))
            })
            .and_then(|v| v.as_str())
            .unwrap_or("forbidden")
            .to_string(),
        _ => "forbidden".to_string(),
    }
}

fn value_to_query(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::McpError;
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn get_item_sends_agent_header_and_returns_json() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/items/42"))
            .and(header("authorization", "Agent tok"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 42})))
            .mount(&server)
            .await;
        let c = RestClient::new(Some(server.uri()), Some("tok".into()));
        let v = c.get_item(42).await.unwrap();
        assert_eq!(v["id"], 42);
    }

    #[tokio::test]
    async fn maps_404_to_item_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/items/1"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let c = RestClient::new(Some(server.uri()), Some("tok".into()));
        let err = c.get_item(1).await.unwrap_err();
        assert_eq!(err.error, McpError::ItemNotFound);
    }

    #[tokio::test]
    async fn maps_order_404_to_order_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/orders/1"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let c = RestClient::new(Some(server.uri()), Some("tok".into()));
        let err = c.get_order(1).await.unwrap_err();
        assert_eq!(err.error, McpError::OrderNotFound);
        assert!(err.message.to_lowercase().contains("order"));
    }

    #[tokio::test]
    async fn maps_403_insufficient_scope() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/wallet"))
            .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
                "detail": {"error": "insufficient_scope", "required": ["payment:spend"]}
            })))
            .mount(&server)
            .await;
        let c = RestClient::new(Some(server.uri()), Some("tok".into()));
        let err = c.get_wallet().await.unwrap_err();
        assert_eq!(err.error, McpError::InsufficientScope);
        assert!(err.message.contains("payment:spend"));
    }

    #[tokio::test]
    async fn maps_403_ownership_to_forbidden() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/v1/items/87"))
            .respond_with(
                ResponseTemplate::new(403).set_body_json(serde_json::json!({"detail": "Not your item"})),
            )
            .mount(&server)
            .await;
        let c = RestClient::new(Some(server.uri()), Some("tok".into()));
        let err = c.update_item(87, &serde_json::json!({"title": "x"})).await.unwrap_err();
        assert_eq!(err.error, McpError::Forbidden);
        assert!(err.message.contains("Not your item"));
    }

    #[tokio::test]
    async fn maps_429_with_retry_after() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/wallet"))
            .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "30"))
            .mount(&server)
            .await;
        let c = RestClient::new(Some(server.uri()), Some("tok".into()));
        let err = c.get_wallet().await.unwrap_err();
        assert_eq!(err.error, McpError::RateLimited);
        assert!(err.message.contains("30"));
    }

    #[tokio::test]
    async fn delist_item_issues_delete_and_synthesizes_result() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/v1/items/94"))
            .and(header("authorization", "Agent tok"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        let c = RestClient::new(Some(server.uri()), Some("tok".into()));
        let v = c.delist_item(94).await.unwrap();
        assert_eq!(v["item_id"], 94);
        assert_eq!(v["status"], "delisted");
    }

    #[tokio::test]
    async fn malformed_token_does_not_panic() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/items/1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 1})))
            .mount(&server)
            .await;
        let c = RestClient::new(Some(server.uri()), Some("bad\ntoken".into()));
        // Before the fix this PANICS while building the client; after, the header is
        // simply omitted and the request succeeds (mock returns 200).
        let r = c.get_item(1).await;
        assert!(r.is_ok(), "expected graceful result, got {r:?}");
    }

    #[tokio::test]
    async fn deliver_credential_posts_wrapped_body() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/orders/7/deliver"))
            .and(body_json(
                serde_json::json!({"credential": {"api_key": "x"}}),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
            .mount(&server)
            .await;
        let c = RestClient::new(Some(server.uri()), Some("tok".into()));
        let v = c
            .deliver_credential(7, &serde_json::json!({"api_key": "x"}))
            .await
            .unwrap();
        assert_eq!(v["ok"], true);
    }

    #[tokio::test]
    async fn reveal_credential_posts_to_reveal_path() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/orders/9/reveal-credential"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"credential": {}})),
            )
            .mount(&server)
            .await;
        let c = RestClient::new(Some(server.uri()), Some("tok".into()));
        let v = c.reveal_credential(9).await.unwrap();
        assert!(v.get("credential").is_some());
    }
}
