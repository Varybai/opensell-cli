use clap::{builder::PossibleValue, Arg, ArgAction};
use serde_json::Value;

// clap 4 stores owned values (Id/Str/StyledStr/OsStr all impl From<String>), so
// no 'static leaking is needed — pass owned Strings straight through.
pub fn args_for_schema(schema: &Value) -> Vec<Arg> {
    let props = schema.get("properties").and_then(|p| p.as_object());
    let required: Vec<&str> = schema
        .get("required")
        .and_then(|r| r.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();
    let mut out = Vec::new();
    let Some(props) = props else {
        return out;
    };
    for (name, spec) in props {
        let ty = spec
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("string");
        let help_text = spec
            .get("description")
            .and_then(|d| d.as_str())
            .unwrap_or("")
            .to_string();
        let long_form = name.replace('_', "-");
        let has_default = spec.get("default").is_some();
        let is_required = required.contains(&name.as_str()) && !has_default;

        let mut arg = Arg::new(name.clone()).long(long_form).help(help_text);
        match ty {
            "boolean" => {
                arg = arg.action(ArgAction::SetTrue);
            }
            "array" => {
                arg = arg.action(ArgAction::Append).required(is_required);
            }
            _ => {
                arg = arg.action(ArgAction::Set).required(is_required);
                if let Some(en) = spec.get("enum").and_then(|e| e.as_array()) {
                    let allowed: Vec<PossibleValue> = en
                        .iter()
                        .filter_map(|v| v.as_str().map(|s| PossibleValue::new(s.to_string())))
                        .collect();
                    arg = arg.value_parser(allowed);
                }
                if let Some(def) = spec.get("default") {
                    let d_str = match def {
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    arg = arg.default_value(d_str);
                }
            }
        }
        out.push(arg);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn required_int_becomes_required_kebab_flag() {
        let schema = json!({"type":"object","properties":{"item_id":{"type":"integer"}},"required":["item_id"]});
        let args = args_for_schema(&schema);
        let a = args.iter().find(|a| a.get_id() == "item_id").unwrap();
        assert!(a.is_required_set());
        assert_eq!(a.get_long(), Some("item-id"));
    }

    #[test]
    fn array_is_multiple() {
        let schema = json!({"type":"object","properties":{"paths":{"type":"array","items":{"type":"string"}}},"required":["paths"]});
        let args = args_for_schema(&schema);
        let a = args.iter().find(|a| a.get_id() == "paths").unwrap();
        assert!(matches!(a.get_action(), clap::ArgAction::Append));
    }

    #[test]
    fn default_makes_optional() {
        let schema =
            json!({"type":"object","properties":{"limit":{"type":"integer","default":20}}});
        let args = args_for_schema(&schema);
        let a = args.iter().find(|a| a.get_id() == "limit").unwrap();
        assert!(!a.is_required_set());
    }
}
