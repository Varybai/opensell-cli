use crate::errors::ToolError;
use crate::rest_client::RestClient;
use serde_json::{json, Map, Value};

fn req_i64(args: &Map<String, Value>, k: &str) -> Result<i64, ToolError> {
    args.get(k)
        .and_then(|v| v.as_i64())
        .ok_or_else(|| ToolError::invalid_args(&format!("missing or invalid {k}")))
}
fn req_str<'a>(args: &'a Map<String, Value>, k: &str) -> Result<&'a str, ToolError> {
    args.get(k)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::invalid_args(&format!("missing or invalid {k}")))
}
fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::String(s) => !s.is_empty(),
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

pub async fn run_tool(
    client: &RestClient,
    name: &str,
    args: &Map<String, Value>,
) -> Result<Value, ToolError> {
    match name {
        "ping" => client.ping().await,
        "search_items" => {
            let mut params = Map::new();
            // q -> keyword. Filter on null (not truthiness) so a legitimate zero
            // value — e.g. min_price=0 or an explicit limit — is not dropped (L4).
            if let Some(v) = args.get("q") {
                if !v.is_null() {
                    params.insert("keyword".into(), v.clone());
                }
            }
            for k in ["category_id", "min_price", "max_price", "sort", "limit"] {
                if let Some(v) = args.get(k) {
                    if !v.is_null() {
                        params.insert(k.into(), v.clone());
                    }
                }
            }
            client.search_items(&Value::Object(params)).await
        }
        "get_item" => client.get_item(req_i64(args, "item_id")?).await,
        "list_categories" => client.list_categories().await,
        "get_order" => client.get_order(req_i64(args, "order_id")?).await,
        "get_wallet" => client.get_wallet().await,
        "list_conversations" => client.list_conversations().await,
        "get_messages" => {
            let id = req_i64(args, "conversation_id")?;
            let mut params = Map::new();
            for k in ["limit", "before"] {
                if let Some(v) = args.get(k) {
                    if !v.is_null() {
                        params.insert(k.into(), v.clone());
                    }
                }
            }
            client.get_messages(id, &Value::Object(params)).await
        }
        "contact_seller" => {
            client
                .contact_seller(req_i64(args, "item_id")?, req_str(args, "message")?)
                .await
        }
        "send_message" => {
            client
                .send_message(req_i64(args, "conversation_id")?, req_str(args, "content")?)
                .await
        }
        "publish_item" => {
            let mut payload = Map::new();
            payload.insert("title".into(), json!(req_str(args, "title")?));
            payload.insert("description".into(), json!(req_str(args, "description")?));
            payload.insert(
                "price".into(),
                args.get("price")
                    .cloned()
                    .ok_or_else(|| ToolError::invalid_args("missing price"))?,
            );
            payload.insert("category_id".into(), json!(req_i64(args, "category_id")?));
            for k in ["condition", "structured_attributes", "image_ids"] {
                if let Some(v) = args.get(k) {
                    if truthy(v) {
                        payload.insert(k.into(), v.clone());
                    }
                }
            }
            client.publish_item(&Value::Object(payload)).await
        }
        "update_item" => {
            let id = req_i64(args, "item_id")?;
            let updates: Map<String, Value> = args
                .iter()
                .filter(|(k, v)| k.as_str() != "item_id" && !v.is_null())
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            client.update_item(id, &Value::Object(updates)).await
        }
        "delist_item" => client.delist_item(req_i64(args, "item_id")?).await,
        "upload_image" => {
            let paths = args
                .get("paths")
                .and_then(|v| v.as_array())
                .ok_or_else(|| ToolError::invalid_args("missing paths"))?;
            let mut files = Vec::new();
            for p in paths {
                let path = p
                    .as_str()
                    .ok_or_else(|| ToolError::invalid_args("path must be string"))?;
                if !std::path::Path::new(path).is_file() {
                    return Err(ToolError::invalid_args(&format!("Image not found: {path}")));
                }
                let data = std::fs::read(path)
                    .map_err(|_| ToolError::invalid_args(&format!("Image not found: {path}")))?;
                let ctype = mime_guess::from_path(path)
                    .first_or_octet_stream()
                    .to_string();
                let fname = std::path::Path::new(path)
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string();
                files.push((fname, data, ctype));
            }
            let ids = client.upload_images(files).await?;
            Ok(json!({"image_ids": ids}))
        }
        "place_order" => {
            let item_id = req_i64(args, "item_id")?;
            let quantity = args.get("quantity").and_then(|v| v.as_i64()).unwrap_or(1);
            client.place_order(item_id, quantity).await
        }
        "pay_order" => client.pay_order(req_i64(args, "order_id")?).await,
        "wallet_withdraw" => {
            let amount = args
                .get("amount")
                .and_then(|v| v.as_f64())
                .ok_or_else(|| ToolError::invalid_args("missing amount"))?;
            client.wallet_withdraw(amount).await
        }
        "reveal_credential" => client.reveal_credential(req_i64(args, "order_id")?).await,
        "confirm_order" => client.confirm_order(req_i64(args, "order_id")?).await,
        "deliver_credential" => {
            let order_id = req_i64(args, "order_id")?;
            let credential = args
                .get("credential")
                .ok_or_else(|| ToolError::invalid_args("missing credential"))?;
            client.deliver_credential(order_id, credential).await
        }
        _ => Err(ToolError::server_error()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rest_client::RestClient;
    use serde_json::{json, Map, Value};
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn args(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    #[tokio::test]
    async fn search_items_maps_q_to_keyword() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/items"))
            .and(query_param("keyword", "phone"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"items": []})))
            .mount(&server)
            .await;
        let c = RestClient::new(Some(server.uri()), Some("t".into()));
        let out = run_tool(&c, "search_items", &args(json!({"q": "phone"})))
            .await
            .unwrap();
        assert!(out.get("items").is_some());
    }

    #[tokio::test]
    async fn unknown_tool_is_server_error() {
        let c = RestClient::new(Some("http://127.0.0.1:0".into()), Some("t".into()));
        let err = run_tool(&c, "nope", &Map::new()).await.unwrap_err();
        assert_eq!(err.error, crate::errors::McpError::ServerError);
    }
}
