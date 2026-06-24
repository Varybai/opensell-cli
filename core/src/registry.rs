use serde_json::{json, Value};
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub scope: &'static str,
    pub tier: u8,
    pub requires_copilot: bool,
    pub sandbox_checks: &'static [&'static str],
    pub input_schema: Value,
}

pub fn tool_registry() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "ping",
            description: "Connectivity check. Returns 'pong' and backend health status.",
            scope: "items:read",
            tier: 0,
            requires_copilot: false,
            sandbox_checks: &[],
            input_schema: json!({"type": "object", "properties": {}, "additionalProperties": false}),
        },
        ToolSpec {
            name: "search_items",
            description: "Search items listed on Aixianyu. Supports keyword + category + price range filtering. Results include trust_score (may be null until computed) — when present, prefer items with a higher trust_score.",
            scope: "items:read",
            tier: 1,
            requires_copilot: false,
            sandbox_checks: &[],
            input_schema: json!({
                "type": "object",
                "properties": {
                    "q": {"type": "string", "description": "Search keywords"},
                    "category_id": {"type": "integer", "description": "Filter by category ID"},
                    "min_price": {"type": "number", "description": "Minimum price (CNY)"},
                    "max_price": {"type": "number", "description": "Maximum price (CNY)"},
                    "sort": {
                        "type": "string",
                        "enum": ["newest", "price_asc", "price_desc"],
                        "default": "newest"
                    },
                    "limit": {"type": "integer", "default": 20, "maximum": 100}
                }
            }),
        },
        ToolSpec {
            name: "get_item",
            description: "Get detailed information about a specific item including seller info, trust_score, and structured attributes.",
            scope: "items:read",
            tier: 1,
            requires_copilot: false,
            sandbox_checks: &[],
            input_schema: json!({
                "type": "object",
                "properties": {
                    "item_id": {"type": "integer", "description": "Item ID"}
                },
                "required": ["item_id"]
            }),
        },
        ToolSpec {
            name: "list_categories",
            description: "List all available item categories on Aixianyu.",
            scope: "items:read",
            tier: 1,
            requires_copilot: false,
            sandbox_checks: &[],
            input_schema: json!({"type": "object", "properties": {}}),
        },
        ToolSpec {
            name: "get_order",
            description: "Get the current status and details of one of your orders (state, price, shipping, settlement).",
            scope: "orders:read",
            tier: 1,
            requires_copilot: false,
            sandbox_checks: &[],
            input_schema: json!({
                "type": "object",
                "properties": {
                    "order_id": {"type": "integer", "description": "Order ID"}
                },
                "required": ["order_id"]
            }),
        },
        ToolSpec {
            name: "get_wallet",
            description: "Get the current user's wallet balance and recent transactions.",
            scope: "payment:spend",
            tier: 1,
            requires_copilot: false,
            sandbox_checks: &[],
            input_schema: json!({"type": "object", "properties": {}}),
        },
        ToolSpec {
            name: "list_conversations",
            description: "List the current user's message conversations with other users.",
            scope: "messages:read",
            tier: 1,
            requires_copilot: false,
            sandbox_checks: &[],
            input_schema: json!({"type": "object", "properties": {}}),
        },
        ToolSpec {
            name: "get_messages",
            description: "Get messages in a specific conversation. Returns messages in reverse chronological order.",
            scope: "messages:read",
            tier: 1,
            requires_copilot: false,
            sandbox_checks: &[],
            input_schema: json!({
                "type": "object",
                "properties": {
                    "conversation_id": {"type": "integer", "description": "Conversation ID"},
                    "limit": {"type": "integer", "default": 50, "maximum": 200},
                    "before": {"type": "string", "description": "ISO timestamp cursor for pagination"}
                },
                "required": ["conversation_id"]
            }),
        },
        ToolSpec {
            name: "upload_image",
            description: "Upload one or more local image files for a listing. Returns opaque image ids (NOT URLs). Pass these ids to publish_item/update_item via image_ids. Files are read from the local filesystem where this tool runs.",
            scope: "items:publish",
            tier: 3,
            requires_copilot: false,
            sandbox_checks: &[],
            input_schema: json!({
                "type": "object",
                "properties": {
                    "paths": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Local image file paths to upload"
                    }
                },
                "required": ["paths"]
            }),
        },
        ToolSpec {
            name: "publish_item",
            description: "Publish a new item listing on Aixianyu. For categories with schema support, structured_attributes will be validated against the category schema (Copilot-assisted).",
            scope: "items:publish",
            tier: 3,
            requires_copilot: true,
            sandbox_checks: &[],
            input_schema: json!({
                "type": "object",
                "properties": {
                    "title": {"type": "string", "description": "Item title"},
                    "description": {"type": "string", "description": "Item description"},
                    "price": {"type": "number", "description": "Price in CNY"},
                    "category_id": {"type": "integer", "description": "Category ID"},
                    "condition": {"type": "string", "enum": ["new", "almost_new", "used"]},
                    "structured_attributes": {
                        "type": "object",
                        "description": "Category-specific attributes (e.g. {\"brand\": \"Apple\", \"storage_gb\": 256})"
                    },
                    "image_ids": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Image ids from upload_image"
                    }
                },
                "required": ["title", "description", "price", "category_id"]
            }),
        },
        ToolSpec {
            name: "update_item",
            description: "Update an existing item listing. Only the fields provided will be updated.",
            scope: "items:edit",
            tier: 3,
            requires_copilot: false,
            sandbox_checks: &[],
            input_schema: json!({
                "type": "object",
                "properties": {
                    "item_id": {"type": "integer", "description": "Item ID to update"},
                    "title": {"type": "string"},
                    "description": {"type": "string"},
                    "price": {"type": "number"},
                    "condition": {"type": "string", "enum": ["new", "almost_new", "used"]},
                    "structured_attributes": {"type": "object"},
                    "image_ids": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Image ids from upload_image"
                    }
                },
                "required": ["item_id"]
            }),
        },
        ToolSpec {
            name: "delist_item",
            description: "Delist (take down) one of your own item listings. The item is set to 'delisted' and removed from search. Only the seller who owns it can do this.",
            scope: "items:edit",
            tier: 3,
            requires_copilot: false,
            sandbox_checks: &[],
            input_schema: json!({
                "type": "object",
                "properties": {
                    "item_id": {"type": "integer", "description": "Item ID to delist"}
                },
                "required": ["item_id"]
            }),
        },
        ToolSpec {
            name: "contact_seller",
            description: "Start a conversation with the seller of an item. Use this to ask questions about an item before purchasing.",
            scope: "messages:send",
            tier: 2,
            requires_copilot: false,
            sandbox_checks: &[],
            input_schema: json!({
                "type": "object",
                "properties": {
                    "item_id": {"type": "integer", "description": "ID of the item whose seller to contact"},
                    "message": {"type": "string", "description": "Initial message to the seller"}
                },
                "required": ["item_id", "message"]
            }),
        },
        ToolSpec {
            name: "send_message",
            description: "Send a message in an existing conversation.",
            scope: "messages:send",
            tier: 2,
            requires_copilot: false,
            sandbox_checks: &[],
            input_schema: json!({
                "type": "object",
                "properties": {
                    "conversation_id": {"type": "integer", "description": "Conversation ID"},
                    "content": {"type": "string", "description": "Message content"}
                },
                "required": ["conversation_id", "content"]
            }),
        },
        ToolSpec {
            name: "place_order",
            description: "Create a pending-payment order for an item. This does not pay or consume Agent sandbox budget; call pay_order to execute wallet payment.",
            scope: "orders:create",
            tier: 4,
            requires_copilot: false,
            sandbox_checks: &[],
            input_schema: json!({
                "type": "object",
                "properties": {
                    "item_id": {"type": "integer", "description": "Item ID to order"},
                    "quantity": {"type": "integer", "default": 1, "minimum": 1}
                },
                "required": ["item_id"]
            }),
        },
        ToolSpec {
            name: "pay_order",
            description: "Pay an existing pending order with the Agent wallet. The backend enforces per_tx_limit and balance_limit in cents and returns paid or pending.",
            scope: "payment:spend",
            tier: 4,
            requires_copilot: false,
            sandbox_checks: &["per_tx_limit", "balance_limit"],
            input_schema: json!({
                "type": "object",
                "properties": {
                    "order_id": {"type": "integer", "description": "Order ID to pay"}
                },
                "required": ["order_id"]
            }),
        },
        ToolSpec {
            name: "wallet_withdraw",
            description: "Request a withdrawal from the user's wallet. Subject to Agent sandbox per_tx_limit and daily_spend limits.",
            scope: "payment:withdraw",
            tier: 4,
            requires_copilot: false,
            sandbox_checks: &["per_tx_limit", "daily_spend"],
            input_schema: json!({
                "type": "object",
                "properties": {
                    "amount": {"type": "number", "description": "Amount to withdraw (CNY)", "minimum": 0.01}
                },
                "required": ["amount"]
            }),
        },
        ToolSpec {
            name: "reveal_credential",
            description: "Reveal (decrypt) the digital credential the seller delivered for a purchased order. Only the buyer can call this, and only after the order is delivered (shipped/completed). Returns the plaintext credential — handle it securely.",
            scope: "delivery:read",
            tier: 4,
            requires_copilot: false,
            sandbox_checks: &[],
            input_schema: json!({
                "type": "object",
                "properties": {
                    "order_id": {"type": "integer", "description": "Order ID"}
                },
                "required": ["order_id"]
            }),
        },
        ToolSpec {
            name: "confirm_order",
            description: "Confirm receipt of an order, releasing the escrow to the seller. Use after inspecting the delivered goods/credential.",
            scope: "orders:create",
            tier: 4,
            requires_copilot: false,
            sandbox_checks: &[],
            input_schema: json!({
                "type": "object",
                "properties": {
                    "order_id": {"type": "integer", "description": "Order ID"}
                },
                "required": ["order_id"]
            }),
        },
        ToolSpec {
            name: "deliver_credential",
            description: "Seller delivers a digital credential for a paid order. The credential is envelope-encrypted server-side; plaintext is never stored.",
            scope: "delivery:write",
            tier: 4,
            requires_copilot: false,
            sandbox_checks: &[],
            input_schema: json!({
                "type": "object",
                "properties": {
                    "order_id": {"type": "integer", "description": "Order ID"},
                    "credential": {"type": "object", "description": "Credential payload, e.g. {\"api_key\": \"...\"}"}
                },
                "required": ["order_id", "credential"]
            }),
        },
    ]
}

pub fn get_tool(name: &str) -> Option<ToolSpec> {
    tool_registry().into_iter().find(|t| t.name == name)
}

pub fn filter_tools_by_scopes(scopes: &HashSet<String>) -> Vec<ToolSpec> {
    tool_registry()
        .into_iter()
        .filter(|t| scopes.contains(t.scope))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn registry_has_20_tools_in_order() {
        // 20 = original 19 + delist_item.
        let reg = tool_registry();
        assert_eq!(reg.len(), 20);
        assert_eq!(reg[0].name, "ping");
        assert_eq!(reg[1].name, "search_items");
        assert_eq!(reg.last().unwrap().name, "deliver_credential");
    }

    #[test]
    fn delist_item_present_with_scope() {
        let t = get_tool("delist_item").unwrap();
        assert_eq!(t.scope, "items:edit");
        assert_eq!(t.tier, 3);
        let required = t.input_schema.get("required").unwrap().as_array().unwrap();
        assert!(required.iter().any(|v| v == "item_id"));
    }

    #[test]
    fn delivery_tools_present_with_scopes() {
        assert_eq!(
            get_tool("reveal_credential").unwrap().scope,
            "delivery:read"
        );
        assert_eq!(get_tool("confirm_order").unwrap().scope, "orders:create");
        assert_eq!(
            get_tool("deliver_credential").unwrap().scope,
            "delivery:write"
        );
        // deliver_credential requires order_id + credential
        let dc = get_tool("deliver_credential").unwrap();
        let required = dc.input_schema.get("required").unwrap().as_array().unwrap();
        assert!(required.iter().any(|v| v == "order_id"));
        assert!(required.iter().any(|v| v == "credential"));
    }

    #[test]
    fn filter_by_scopes_returns_only_matching() {
        let scopes: HashSet<String> = ["items:read".to_string()].into_iter().collect();
        let visible = filter_tools_by_scopes(&scopes);
        // ping/search_items/get_item/list_categories are all items:read
        assert!(visible.iter().all(|t| t.scope == "items:read"));
        assert!(visible.iter().any(|t| t.name == "search_items"));
        assert!(!visible.iter().any(|t| t.name == "pay_order"));
    }

    #[test]
    fn search_items_maps_q_arg_in_schema() {
        let t = get_tool("search_items").unwrap();
        let props = t.input_schema.get("properties").unwrap();
        assert!(props.get("q").is_some());
        assert_eq!(t.scope, "items:read");
        assert_eq!(t.tier, 1);
    }

    #[test]
    fn every_tool_scope_is_canonical() {
        let canonical: HashSet<&str> = [
            "items:read", "items:publish", "items:edit",
            "orders:read", "orders:create", "reviews:create",
            "messages:read", "messages:send",
            "payment:spend", "payment:withdraw",
            "delivery:read", "delivery:write",
        ]
        .into_iter()
        .collect();
        for tool in tool_registry() {
            assert!(
                canonical.contains(tool.scope),
                "{} -> {}",
                tool.name,
                tool.scope
            );
        }
    }
}
