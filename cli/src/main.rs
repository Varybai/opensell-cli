mod schema_to_clap;

use clap::{Arg, ArgAction, ArgMatches, Command};
use opensell_core::{
    handlers::run_tool,
    registry::{get_tool, tool_registry, ToolSpec},
    rest_client::RestClient,
};
use schema_to_clap::args_for_schema;
use serde_json::{json, Map, Value};

fn build_cli() -> Command {
    let mut app = Command::new("opensell")
        .about("OPENSELL marketplace CLI for AI agents.")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .arg(
            Arg::new("token")
                .long("token")
                .global(true)
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new("base_url")
                .long("base-url")
                .global(true)
                .action(ArgAction::Set),
        );
    for spec in tool_registry() {
        let sub = Command::new(spec.name.replace('_', "-"))
            .about(format!(
                "{}\n\nRequires scope: {}",
                spec.description, spec.scope
            ))
            // Accept leading-dash numbers as option values (e.g. --min-price -50,
            // --limit -1) instead of mistaking them for unknown flags (L3).
            .allow_negative_numbers(true)
            .args(args_for_schema(&spec.input_schema));
        app = app.subcommand(sub);
    }
    app = app.subcommand(
        Command::new("catalog").about("Emit the full command surface as JSON (for agents)."),
    );
    app
}

fn collect_args(spec: &ToolSpec, m: &ArgMatches) -> Result<Map<String, Value>, String> {
    let props = spec
        .input_schema
        .get("properties")
        .and_then(|p| p.as_object());
    let mut args = Map::new();
    let Some(props) = props else {
        return Ok(args);
    };
    for (name, pspec) in props {
        let ty = pspec
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("string");
        match ty {
            "boolean" => {
                if m.get_flag(name) {
                    args.insert(name.clone(), json!(true));
                }
            }
            "array" => {
                if let Some(vals) = m.get_many::<String>(name) {
                    let item_ty = pspec
                        .get("items")
                        .and_then(|i| i.get("type"))
                        .and_then(|t| t.as_str())
                        .unwrap_or("string");
                    let arr: Vec<Value> = vals.map(|s| parse_scalar(s, item_ty)).collect();
                    if !arr.is_empty() {
                        args.insert(name.clone(), Value::Array(arr));
                    }
                }
            }
            "object" => {
                if let Some(s) = m.get_one::<String>(name) {
                    let v: Value = serde_json::from_str(s).map_err(|e| {
                        format!(
                            "--{}: must be a valid JSON object ({e})",
                            name.replace('_', "-")
                        )
                    })?;
                    args.insert(name.clone(), v);
                }
            }
            other => {
                if let Some(s) = m.get_one::<String>(name) {
                    args.insert(name.clone(), parse_scalar(s, other));
                }
            }
        }
    }
    Ok(args)
}

fn parse_scalar(s: &str, ty: &str) -> Value {
    match ty {
        "integer" => s
            .parse::<i64>()
            .map(|n| json!(n))
            .unwrap_or_else(|_| json!(s)),
        "number" => s
            .parse::<f64>()
            .map(|n| json!(n))
            .unwrap_or_else(|_| json!(s)),
        _ => json!(s),
    }
}

fn build_catalog() -> Value {
    let commands: Vec<Value> = tool_registry()
        .into_iter()
        .map(|t| {
            let props = t
                .input_schema
                .get("properties")
                .and_then(|p| p.as_object())
                .cloned()
                .unwrap_or_default();
            let required: Vec<&str> = t
                .input_schema
                .get("required")
                .and_then(|r| r.as_array())
                .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
                .unwrap_or_default();
            let args: Vec<Value> = props
                .iter()
                .map(|(pn, ps)| {
                    json!({
                        "name": pn,
                        "type": ps.get("type").and_then(|x| x.as_str()).unwrap_or("string"),
                        "required": required.contains(&pn.as_str()),
                        "default": ps.get("default").cloned().unwrap_or(Value::Null),
                    })
                })
                .collect();
            json!({
                "command": t.name.replace('_', "-"),
                "tool": t.name,
                "scope": t.scope,
                "tier": t.tier,
                "description": t.description,
                "args": args,
            })
        })
        .collect();
    json!({ "commands": commands })
}

fn main() {
    let app = build_cli();
    // Don't let clap's default exit code (2) collide with UNAUTHORIZED=2 (M1).
    // --help/--version surface as "errors" → print and exit 0; genuine usage
    // errors exit 64 (EX_USAGE), distinct from the MCPError codes (1-9).
    let matches = match app.try_get_matches() {
        Ok(m) => m,
        Err(e) => {
            let _ = e.print();
            use clap::error::ErrorKind;
            match e.kind() {
                ErrorKind::DisplayHelp
                | ErrorKind::DisplayVersion
                | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => std::process::exit(0),
                _ => std::process::exit(64),
            }
        }
    };

    let token = matches
        .get_one::<String>("token")
        .cloned()
        .or_else(|| std::env::var("AIXIANYU_AGENT_TOKEN").ok())
        .unwrap_or_default();
    let base_url = matches
        .get_one::<String>("base_url")
        .cloned()
        .or_else(|| std::env::var("AIXIANYU_BASE_URL").ok());

    let (sub, sub_m) = matches.subcommand().expect("subcommand required");

    if sub == "catalog" {
        println!(
            "{}",
            serde_json::to_string_pretty(&build_catalog()).unwrap()
        );
        return;
    }

    let tool_name = sub.replace('-', "_");
    let spec = get_tool(&tool_name).expect("known subcommand");

    let args = match collect_args(&spec, sub_m) {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("{}", json!({"error": "INVALID_ARGS", "message": msg}));
            std::process::exit(1);
        }
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let client = RestClient::new(base_url, Some(token));
    match rt.block_on(run_tool(&client, &tool_name, &args)) {
        Ok(v) => {
            println!("{}", serde_json::to_string(&v).unwrap());
        }
        Err(e) => {
            eprintln!("{}", e.to_json());
            std::process::exit(e.error.exit_code());
        }
    }
}
