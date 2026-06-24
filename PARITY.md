# Parity Evidence: Rust CLI vs Python CLI

This document records the behavioral parity verification between the Rust
implementation (`rust/`) and the Python implementation (`opensell-cli/` +
`mcp-server/`) as of version 0.2.0.

---

## 1. Catalog JSON diff

### Method

Both CLIs expose a `catalog` sub-command that emits the full machine-readable
command surface as JSON (`{"commands": [...]}`) — one entry per tool in the
shared registry.

```bash
# Rust catalog
source "$HOME/.cargo/env" && cd rust && cargo run -q -p opensell-cli -- catalog > /tmp/rs_catalog.json

# Python catalog (needs click + aixianyu_mcp on PYTHONPATH)
python3 -m venv /tmp/osv_py && /tmp/osv_py/bin/pip install --quiet click
PYTHONPATH="mcp-server:opensell-cli" /tmp/osv_py/bin/python \
  -c "from opensell_cli.catalog import build_catalog, json; print(json.dumps(build_catalog(), ensure_ascii=False))" \
  > /tmp/py_catalog.json

# Normalize: sort top-level keys, then diff
python3 -c "
import json
a = json.load(open('/tmp/rs_catalog.json'))
b = json.load(open('/tmp/py_catalog.json'))
ra = json.dumps(a, sort_keys=True, ensure_ascii=False)
rb = json.dumps(b, sort_keys=True, ensure_ascii=False)
print('IDENTICAL' if ra == rb else 'DIFF')
open('/tmp/rs_norm.json','w').write(json.dumps(a, sort_keys=True, indent=2, ensure_ascii=False))
open('/tmp/py_norm.json','w').write(json.dumps(b, sort_keys=True, indent=2, ensure_ascii=False))
"
diff /tmp/py_norm.json /tmp/rs_norm.json
```

### Result: IDENTICAL modulo args-array ordering

- **With args-array order normalized** (each command's `args` list sorted by
  `name` before comparison): **IDENTICAL** — zero diff.
- **Without args normalization**: the only differences are the ordering of
  `args` entries within each command.

### Root cause of ordering difference

| Layer | Ordering |
|---|---|
| Python `TOOL_REGISTRY` (`registry.py`) | `dict`, preserves insertion order |
| Python `build_catalog` | iterates `props.items()` → insertion order |
| Rust `tool_registry()` | `Vec<ToolSpec>`, same insertion order as Python |
| Rust `build_catalog` | iterates `props` from `serde_json::json!{}` |
| `serde_json::json!{}` macro | underlying `serde_json::Map` is a `BTreeMap` → alphabetical |

The args-array ordering difference is **structural, not semantic**. All field
values (`name`, `type`, `required`, `default`) are identical across both
implementations.

**Conclusion: no real parity gap in the catalog surface.**

---

## 2. Exit-code table

Both implementations use the same 8-code table (Python: `opensell_cli/output.py`
`EXIT_CODES`; Rust: `core/src/errors.rs` `McpError::exit_code()`):

| Code | Error name | Python `EXIT_CODES` | Rust `exit_code()` |
|------|------------|--------------------|--------------------|
| 1 | `INVALID_ARGS` | ✅ | ✅ |
| 2 | `UNAUTHORIZED` | ✅ | ✅ |
| 3 | `INSUFFICIENT_SCOPE` | ✅ | ✅ |
| 4 | `AGENT_SANDBOX_LIMIT` | ✅ | ✅ |
| 5 | `INSUFFICIENT_BALANCE` | ✅ | ✅ |
| 6 | `RATE_LIMITED` | ✅ | ✅ |
| 7 | `ITEM_NOT_FOUND` | ✅ | ✅ |
| 7 | `CONVERSATION_NOT_FOUND` | ✅ | ✅ (same code as `ITEM_NOT_FOUND`) |
| 8 | `SERVER_ERROR` | ✅ | ✅ |

Covered by `core/src/errors.rs` unit test `exit_codes_match_table`.

### Known plan-mandated divergence

| Scenario | Python exit code | Rust exit code | Reason |
|---|---|---|---|
| Malformed object-arg JSON (e.g. `--structured-attributes "{not json}"`) | **2** (Click usage error) | **1** (`INVALID_ARGS`) | Click catches the parse error before dispatch; Rust detects it in `collect_args` and emits `INVALID_ARGS` exit 1. Documented in task spec as acceptable. |

---

## 3. Environment variables

Both implementations read the same two env vars:

| Variable | Purpose |
|---|---|
| `AIXIANYU_BASE_URL` | Base URL for the REST API (default: `https://varybai.online/api`) |
| `AIXIANYU_AGENT_TOKEN` | Agent token sent as `Authorization: Agent <token>` |

CLI flags `--base-url` / `--token` override env vars in both implementations.

---

## 4. REST endpoint checklist

All 19 tools map to the same REST endpoints in both implementations
(Python: `mcp-server/aixianyu_mcp/rest_client.py`;
Rust: `rust/core/src/rest_client.rs`):

| Tool | Method | Path | Python ✅ | Rust ✅ |
|---|---|---|---|---|
| `ping` | GET | `/health` | ✅ | ✅ |
| `search_items` | GET | `/v1/items` | ✅ | ✅ |
| `get_item` | GET | `/v1/items/{item_id}` | ✅ | ✅ |
| `list_categories` | GET | `/v1/categories` | ✅ | ✅ |
| `get_order` | GET | `/v1/orders/{order_id}` | ✅ | ✅ |
| `get_wallet` | GET | `/v1/wallet` | ✅ | ✅ |
| `list_conversations` | GET | `/v1/conversations` | ✅ | ✅ |
| `get_messages` | GET | `/v1/conversations/{id}/messages` | ✅ | ✅ |
| `contact_seller` | POST | `/v1/conversations/contact` | ✅ | ✅ |
| `send_message` | POST | `/v1/conversations/{id}/messages` | ✅ | ✅ |
| `upload_image` | POST | `/v1/images` (multipart) | ✅ | ✅ |
| `publish_item` | POST | `/v1/items` | ✅ | ✅ |
| `update_item` | PUT | `/v1/items/{item_id}` | ✅ | ✅ |
| `place_order` | POST | `/v1/orders` | ✅ | ✅ |
| `pay_order` | POST | `/v1/orders/{order_id}/pay` | ✅ | ✅ |
| `wallet_withdraw` | POST | `/v1/wallet/withdraw` | ✅ | ✅ |
| `reveal_credential` | POST | `/v1/orders/{order_id}/reveal-credential` | ✅ | ✅ |
| `confirm_order` | POST | `/v1/orders/{order_id}/confirm` | ✅ | ✅ |
| `deliver_credential` | POST | `/v1/orders/{order_id}/deliver` | ✅ | ✅ |

> The last three (digital-credential escrow delivery) were added in the
> `feat/digital-deal` upgrade (main `f37075b`) and introduce two new agent
> scopes — `delivery:read` and `delivery:write` — which the data-driven
> `list_tools`/`filter_tools_by_scopes` honour automatically.

HTTP error mapping (`401`/`403`/`404`/`422`/`429`/other) is identical between
implementations (Task 4).

---

## 5. Handler parity (Task 5)

The following handler-level behaviors were ported and verified:

| Behavior | Python (`execute.py`) | Rust (`handlers.rs`) |
|---|---|---|
| `search_items`: `q` → `keyword` query param | ✅ | ✅ |
| `search_items`: truthy filter (skip empty/null params) | ✅ | ✅ |
| `update_item`: filter out `item_id` + null fields | ✅ | ✅ |
| `upload_image`: return `{"image_ids": [...]}` shape | ✅ | ✅ |
| Truthy check (null/empty-string/zero treated as absent) | ✅ | ✅ |

---

## 6. Tests backing parity

### Rust unit + integration tests

| File | Tests |
|---|---|
| `rust/core/src/errors.rs` | `exit_codes_match_table`, `to_json_matches_python_to_dict`, `names_are_screaming_snake` |
| `rust/core/src/registry.rs` | `registry_has_19_tools_in_order`, `delivery_tools_present_with_scopes`, `filter_by_scopes_returns_only_matching`, `search_items_maps_q_arg_in_schema` |
| `rust/core/src/rest_client.rs` | `get_item_sends_agent_header_and_returns_json`, `maps_404_to_item_not_found`, `maps_429_with_retry_after` |
| `rust/core/src/handlers.rs` | `search_items_maps_q_to_keyword`, `unknown_tool_is_server_error` |
| `rust/cli/tests/e2e.rs` | `catalog_outputs_all_commands`, `missing_required_arg_is_usage_error`, `bad_object_json_exits_invalid_args` |

### Python tests (reference implementation)

| File | Tests |
|---|---|
| `opensell-cli/tests/test_catalog.py` | `test_build_catalog_covers_every_tool`, `test_catalog_entry_shape`, `test_catalog_command_emits_json` |
| `opensell-cli/tests/test_cli_e2e.py` | `test_all_registry_tools_plus_catalog_are_commands`, `test_success_prints_json_and_exit_zero`, `test_backend_error_maps_to_exit_code`, `test_missing_required_arg_is_usage_error`, `test_invalid_object_arg_is_usage_error` |
| `opensell-cli/tests/test_output.py` | Exit-code mapping tests |
| `mcp-server/tests/test_registry.py` | Registry coverage tests |
| `mcp-server/tests/test_error_normalization.py` | HTTP error → `MCPError` mapping |

---

## 7. Summary verdict

| Dimension | Result |
|---|---|
| Catalog JSON (values + fields) | **IDENTICAL** |
| Catalog args-array ordering | **Differs** (Rust: BTreeMap/alphabetical; Python: insertion order) — acceptable, documented above |
| Exit-code table (8 codes) | **IDENTICAL** |
| REST endpoints (19 tools) | **IDENTICAL** |
| Handler behavior | **IDENTICAL** |
| Known divergence | Malformed object JSON → exit 1 (Rust) vs exit 2 (Python); plan-mandated |

**Verdict: parity confirmed.** The Rust implementation faithfully replicates
the Python CLI's command surface, exit codes, REST mapping, and handler
semantics. The args-array ordering difference is a BTreeMap artifact with no
observable effect on consumers.
