use assert_cmd::Command;

#[test]
fn catalog_outputs_all_commands() {
    let mut cmd = Command::cargo_bin("opensell").unwrap();
    let out = cmd.arg("catalog").assert().success();
    let v: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert_eq!(v["commands"].as_array().unwrap().len(), 20);
}

#[test]
fn missing_required_arg_is_usage_error() {
    // clap usage errors exit 64 (EX_USAGE), NOT 2 — 2 is reserved for
    // UNAUTHORIZED so the two can't be confused (M1).
    let mut cmd = Command::cargo_bin("opensell").unwrap();
    cmd.arg("get-item").assert().code(64); // missing --item-id
}

#[test]
fn negative_number_value_is_not_a_flag() {
    // --min-price -50 must parse as a value (L3), not a clap "unexpected
    // argument" (which would be exit 64). With an unreachable backend the
    // request fails -> exit 8 (SERVER_ERROR), proving clap accepted the value.
    let mut cmd = Command::cargo_bin("opensell").unwrap();
    cmd.args([
        "--token",
        "t",
        "--base-url",
        "http://127.0.0.1:1",
        "search-items",
        "--min-price",
        "-50",
        "--limit",
        "5",
    ])
    .assert()
    .code(8);
}

#[test]
fn bad_object_json_exits_invalid_args() {
    let mut cmd = Command::cargo_bin("opensell").unwrap();
    cmd.args([
        "--token",
        "t",
        "--base-url",
        "http://127.0.0.1:1",
        "publish-item",
        "--title",
        "a",
        "--description",
        "b",
        "--price",
        "1",
        "--category-id",
        "1",
        "--structured-attributes",
        "{not json}",
    ])
    .assert()
    .code(1); // INVALID_ARGS
}
