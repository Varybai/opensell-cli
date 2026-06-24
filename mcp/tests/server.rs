//! Smoke test: the binary starts, speaks JSON-RPC over stdio, and answers an
//! `initialize` request with a `result`. Runs with an empty agent token so the
//! scope fetch degrades gracefully (no network). The scope-filter + run_tool
//! logic itself is unit-tested in opensell-core, so it is not re-tested here.

use assert_cmd::Command;

#[test]
fn responds_to_initialize_with_result() {
    let initialize = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}"#;
    // The MCP handshake is two frames: the `initialize` request, then the
    // client's `notifications/initialized`. Without the second, rmcp ends the
    // session with ConnectionClosed("initialize notification") (non-zero exit).
    // Sending both, then EOF, lets the server complete the handshake and exit
    // cleanly. Each frame is one newline-delimited JSON-RPC line.
    let initialized = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
    let stdin = format!("{initialize}\n{initialized}\n");

    let assert = Command::cargo_bin("opensell-mcp")
        .unwrap()
        .env("AIXIANYU_AGENT_TOKEN", "")
        .write_stdin(stdin)
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert!(
        stdout.contains("\"result\""),
        "expected an initialize result in stdout, got: {stdout}"
    );
}
