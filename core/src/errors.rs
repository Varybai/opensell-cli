use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpError {
    InvalidArgs,
    Unauthorized,
    InsufficientScope,
    Forbidden,
    AgentSandboxLimit,
    InsufficientBalance,
    RateLimited,
    ItemNotFound,
    OrderNotFound,
    ConversationNotFound,
    ServerError,
}

impl McpError {
    pub fn name(self) -> &'static str {
        match self {
            McpError::InvalidArgs => "INVALID_ARGS",
            McpError::Unauthorized => "UNAUTHORIZED",
            McpError::InsufficientScope => "INSUFFICIENT_SCOPE",
            McpError::Forbidden => "FORBIDDEN",
            McpError::AgentSandboxLimit => "AGENT_SANDBOX_LIMIT",
            McpError::InsufficientBalance => "INSUFFICIENT_BALANCE",
            McpError::RateLimited => "RATE_LIMITED",
            McpError::ItemNotFound => "ITEM_NOT_FOUND",
            McpError::OrderNotFound => "ORDER_NOT_FOUND",
            McpError::ConversationNotFound => "CONVERSATION_NOT_FOUND",
            McpError::ServerError => "SERVER_ERROR",
        }
    }

    pub fn exit_code(self) -> i32 {
        match self {
            McpError::InvalidArgs => 1,
            McpError::Unauthorized => 2,
            McpError::InsufficientScope => 3,
            McpError::AgentSandboxLimit => 4,
            McpError::InsufficientBalance => 5,
            McpError::RateLimited => 6,
            McpError::ItemNotFound | McpError::OrderNotFound | McpError::ConversationNotFound => 7,
            McpError::ServerError => 8,
            McpError::Forbidden => 9,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ToolError {
    pub error: McpError,
    pub message: String,
}

impl ToolError {
    pub fn new(error: McpError, message: impl Into<String>) -> Self {
        Self {
            error,
            message: message.into(),
        }
    }

    // 固定文案的便捷构造（对应 errors.py 的默认 value）
    pub fn item_not_found() -> Self {
        Self::new(McpError::ItemNotFound, "Item not found or delisted")
    }
    pub fn order_not_found() -> Self {
        Self::new(McpError::OrderNotFound, "Order not found")
    }
    pub fn conversation_not_found() -> Self {
        Self::new(McpError::ConversationNotFound, "Conversation not found")
    }
    pub fn forbidden(reason: &str) -> Self {
        Self::new(McpError::Forbidden, format!("Permission denied: {reason}"))
    }
    pub fn unauthorized() -> Self {
        Self::new(McpError::Unauthorized, "Agent token invalid or expired")
    }
    pub fn server_error() -> Self {
        Self::new(McpError::ServerError, "Platform error. Please retry later")
    }
    pub fn insufficient_scope(required: &str) -> Self {
        Self::new(
            McpError::InsufficientScope,
            format!("Insufficient permissions. Required scope: {required}"),
        )
    }
    pub fn agent_sandbox_limit(limit: &str) -> Self {
        Self::new(
            McpError::AgentSandboxLimit,
            format!("Agent sandbox limit exceeded (limit: {limit})"),
        )
    }
    pub fn rate_limited(retry_after: &str) -> Self {
        Self::new(
            McpError::RateLimited,
            format!("Rate limited. Retry after {retry_after} seconds"),
        )
    }
    pub fn invalid_args(details: &str) -> Self {
        Self::new(
            McpError::InvalidArgs,
            format!("Invalid arguments: {details}"),
        )
    }

    pub fn to_json(&self) -> Value {
        json!({ "error": self.error.name(), "message": self.message })
    }
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
impl std::error::Error for ToolError {}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn to_json_matches_python_to_dict() {
        let e = ToolError::new(McpError::ItemNotFound, "Item not found or delisted");
        assert_eq!(
            e.to_json(),
            json!({"error": "ITEM_NOT_FOUND", "message": "Item not found or delisted"})
        );
    }

    #[test]
    fn exit_codes_match_table() {
        assert_eq!(McpError::InvalidArgs.exit_code(), 1);
        assert_eq!(McpError::Unauthorized.exit_code(), 2);
        assert_eq!(McpError::InsufficientScope.exit_code(), 3);
        assert_eq!(McpError::AgentSandboxLimit.exit_code(), 4);
        assert_eq!(McpError::InsufficientBalance.exit_code(), 5);
        assert_eq!(McpError::RateLimited.exit_code(), 6);
        assert_eq!(McpError::ItemNotFound.exit_code(), 7);
        assert_eq!(McpError::OrderNotFound.exit_code(), 7);
        assert_eq!(McpError::ConversationNotFound.exit_code(), 7);
        assert_eq!(McpError::ServerError.exit_code(), 8);
        assert_eq!(McpError::Forbidden.exit_code(), 9);
    }

    #[test]
    fn names_are_screaming_snake() {
        assert_eq!(McpError::AgentSandboxLimit.name(), "AGENT_SANDBOX_LIMIT");
    }
}
