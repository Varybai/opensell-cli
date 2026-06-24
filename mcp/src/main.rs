//! opensell-mcp: stdio MCP server that reuses opensell-core.
//!
//! Mirrors the Python reference server (`mcp-server/aixianyu_mcp/server.py`):
//! `list_tools` filters by the agent token's scopes (cached, falling back to
//! `{"items:read"}` on error); `call_tool` dispatches `ping` directly and
//! checks scope for everything else before calling `run_tool`. Tool results are
//! wrapped as a single JSON `TextContent`.

use std::collections::HashSet;
use std::sync::Mutex;

use opensell_core::handlers::run_tool;
use opensell_core::registry::{filter_tools_by_scopes, get_tool};
use opensell_core::rest_client::RestClient;

use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, ListToolsResult, PaginatedRequestParams,
    ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{ErrorData, ServerHandler, ServiceExt};

struct OpensellServer {
    client: RestClient,
    cached_scopes: Mutex<Option<HashSet<String>>>,
}

impl OpensellServer {
    /// Resolve the agent token's scopes, caching the first successful (or
    /// fallback) result. On token-introspection failure, degrade to read-only.
    async fn scopes(&self) -> HashSet<String> {
        if let Some(s) = self.cached_scopes.lock().unwrap().clone() {
            return s;
        }
        let scopes = match self.client.get_token_info().await {
            Ok(info) => info["scopes"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
            Err(_) => HashSet::from(["items:read".to_string()]),
        };
        *self.cached_scopes.lock().unwrap() = Some(scopes.clone());
        scopes
    }
}

impl ServerHandler for OpensellServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: rmcp::model::Implementation {
                name: "aixianyu-mcp".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                ..Default::default()
            },
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let scopes = self.scopes().await;
        let tools = filter_tools_by_scopes(&scopes)
            .into_iter()
            .map(|t| {
                let schema = t.input_schema.as_object().cloned().unwrap_or_default();
                Tool::new(t.name, t.description, std::sync::Arc::new(schema))
            })
            .collect();
        Ok(ListToolsResult {
            tools,
            ..Default::default()
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let name = request.name.as_ref();
        let args = request.arguments.unwrap_or_default();

        // `ping` is handled inline so its error shape matches server.py:
        // Ok  → TextContent(json string)
        // Err → TextContent({"error": "<message>"})
        if name == "ping" {
            let text = match run_tool(&self.client, name, &args).await {
                Ok(v) => v.to_string(),
                Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
            };
            return Ok(CallToolResult::success(vec![Content::text(text)]));
        }

        // Scope check (ping already returned above).
        if let Some(spec) = get_tool(name) {
            if !self.scopes().await.contains(spec.scope) {
                let body = serde_json::json!({
                    "error": format!("Insufficient scope. Required: {}", spec.scope)
                });
                return Ok(CallToolResult::success(vec![Content::text(
                    body.to_string(),
                )]));
            }
        }

        let text = match run_tool(&self.client, name, &args).await {
            Ok(v) => v.to_string(),
            Err(e) => e.to_json().to_string(),
        };
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = OpensellServer {
        client: RestClient::from_env(),
        cached_scopes: Mutex::new(None),
    };
    let service = server.serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;
    Ok(())
}
