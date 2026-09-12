//! Deterministic parser for canonical tool syntax.
//!
//! Accepted forms (no LLM):
//! - `todo.add title="Buy milk" deadline=2026-08-18`
//! - `/todo.add title="Buy milk"`
//! - `echo hello` when the spec has `rest_field`
//! - `{"tool":"todo.add","title":"Buy milk"}`

use serde_json::{json, Map, Value};

use crate::schema::ToolSchema;
use crate::spec::ResolvedSpec;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalHit {
    pub tool: &'static str,
    pub input: String,
}

/// Parse `text` as canonical syntax for one of `specs`.
///
/// Returns `None` when this is not structured syntax (leave to NL / chat).
pub fn parse_canonical(text: &str, specs: &[ResolvedSpec]) -> Option<CanonicalHit> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with('{') {
        return parse_json_envelope(trimmed, specs);
    }
    let (token, rest) = split_first_token(trimmed)?;
    let spec = find_spec(specs, token)?;
    let value = parse_args(rest, spec)?;
    if !keys_allowed(spec.schema, &value) {
        return None;
    }
    Some(CanonicalHit {
        tool: spec.name,
        input: value.to_string(),
    })
}

fn find_spec<'a>(specs: &'a [ResolvedSpec], token: &str) -> Option<&'a ResolvedSpec> {
    let mut best: Option<&ResolvedSpec> = None;
    for spec in specs {
        if spec.matches_name(token) && best.map(|b| spec.name.len() > b.name.len()).unwrap_or(true)
        {
            best = Some(spec);
        }
    }
    best
}

fn split_first_token(text: &str) -> Option<(&str, &str)> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    match text.find(char::is_whitespace) {
        Some(i) => Some((text[..i].trim(), text[i..].trim())),
        None => Some((text, "")),
    }
}

fn parse_json_envelope(text: &str, specs: &[ResolvedSpec]) -> Option<CanonicalHit> {
    let mut value: Value = serde_json::from_str(text).ok()?;
    let obj = value.as_object_mut()?;
    let tool = obj
        .remove("tool")
        .or_else(|| obj.remove("name"))
        .and_then(|v| v.as_str().map(|s| s.to_string()))?;
    let spec = find_spec(specs, &tool)?;
    let input = Value::Object(obj.clone());
    if !keys_allowed(spec.schema, &input) {
        return None;
    }
    Some(CanonicalHit {
        tool: spec.name,
        input: input.to_string(),
    })
}

fn parse_args(rest: &str, spec: &ResolvedSpec) -> Option<Value> {
    let rest = rest.trim();
    if rest.is_empty() {
        return Some(json!({}));
    }
    if rest.starts_with('{') {
        let value: Value = serde_json::from_str(rest).ok()?;
        return value.is_object().then_some(value);
    }
    if looks_like_kv(rest) {
        return parse_kv(rest);
    }
    if let Some(field) = spec.rest_field {
        if rest_field_ok(rest) {
            return Some(json!({ field: rest }));
        }
    }
    None
}

fn rest_field_ok(rest: &str) -> bool {
    let t = rest.trim();
    if t.is_empty() {
        return false;
    }
    // Sentences are NL, not canonical positional args.
    if t.ends_with('?') || t.contains(',') {
        return false;
    }
    let words = t.split_whitespace().count();
    words <= 12
}

fn looks_like_kv(rest: &str) -> bool {
    let rest = rest.trim();
    let eq = match rest.find('=') {
        Some(i) => i,
        None => return false,
    };
    is_ident(rest[..eq].trim())
}

fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn parse_kv(rest: &str) -> Option<Value> {
    let mut obj = Map::new();
    let mut i = 0;
    let bytes = rest.as_bytes();
    while i < rest.len() {
        while i < rest.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= rest.len() {
            break;
        }
        let key_start = i;
        while i < rest.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
            i += 1;
        }
        let key = rest[key_start..i].trim();
        if key.is_empty() || !is_ident(key) {
            return None;
        }
        while i < rest.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= rest.len() || bytes[i] != b'=' {
            return None;
        }
        i += 1;
        while i < rest.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let (value, next) = parse_kv_value(rest, i)?;
        obj.insert(key.to_string(), value);
        i = next;
    }
    Some(Value::Object(obj))
}

fn parse_kv_value(src: &str, start: usize) -> Option<(Value, usize)> {
    let bytes = src.as_bytes();
    if start >= src.len() {
        return Some((Value::String(String::new()), start));
    }
    let quote = bytes[start];
    if quote == b'"' || quote == b'\'' {
        let mut i = start + 1;
        let mut out = String::new();
        while i < src.len() {
            let c = bytes[i];
            if c == b'\\' && i + 1 < src.len() {
                out.push(src.as_bytes()[i + 1] as char);
                i += 2;
                continue;
            }
            if c == quote {
                return Some((coerce_scalar(&out), i + 1));
            }
            out.push(src[i..].chars().next()?);
            i += src[i..].chars().next()?.len_utf8();
        }
        return None;
    }
    let mut i = start;
    while i < src.len() && !bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    Some((coerce_scalar(src[start..i].trim()), i))
}

fn coerce_scalar(raw: &str) -> Value {
    if raw.eq_ignore_ascii_case("true") {
        return Value::Bool(true);
    }
    if raw.eq_ignore_ascii_case("false") {
        return Value::Bool(false);
    }
    if let Ok(n) = raw.parse::<i64>() {
        return json!(n);
    }
    if let Ok(n) = raw.parse::<f64>() {
        if raw.contains('.') {
            return json!(n);
        }
    }
    Value::String(raw.to_string())
}

fn keys_allowed(schema: Option<&ToolSchema>, value: &Value) -> bool {
    let Some(schema) = schema else {
        return true;
    };
    if schema.fields.is_empty() {
        return true;
    }
    let Some(obj) = value.as_object() else {
        return false;
    };
    obj.keys().all(|k| schema.field(k).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{AskKind, FieldSpec, ToolSchema};
    use crate::spec::{ResolvedSpec, RespondMode, Safety};

    const TODO_FIELDS: &[FieldSpec] = &[
        FieldSpec {
            name: "title",
            label: "task",
            required: true,
            memory_keys: &[],
            ask_kind: AskKind::Text,
            choices: &[],
        },
        FieldSpec {
            name: "deadline",
            label: "deadline",
            required: false,
            memory_keys: &[],
            ask_kind: AskKind::Text,
            choices: &[],
        },
        FieldSpec {
            name: "priority",
            label: "priority",
            required: false,
            memory_keys: &[],
            ask_kind: AskKind::Text,
            choices: &[],
        },
    ];

    const TODO_SCHEMA: ToolSchema = ToolSchema {
        tool: "todo.add",
        fields: TODO_FIELDS,
    };

    const ECHO_FIELDS: &[FieldSpec] = &[FieldSpec {
        name: "text",
        label: "text",
        required: true,
        memory_keys: &[],
        ask_kind: AskKind::Text,
        choices: &[],
    }];

    const ECHO_SCHEMA: ToolSchema = ToolSchema {
        tool: "echo",
        fields: ECHO_FIELDS,
    };

    fn specs() -> Vec<ResolvedSpec> {
        vec![
            ResolvedSpec {
                name: "todo.add",
                description: "create a task",
                example: r#"todo.add title="Buy milk""#,
                schema: Some(&TODO_SCHEMA),
                aliases: &["/todo"],
                rest_field: Some("title"),
                safety: Safety::Immediate,
                respond: RespondMode::Passthrough,
                likely: &[],
                extract: None,
            },
            ResolvedSpec {
                name: "echo",
                description: "echo text",
                example: r#"echo text="hi""#,
                schema: Some(&ECHO_SCHEMA),
                aliases: &[],
                rest_field: Some("text"),
                safety: Safety::Immediate,
                respond: RespondMode::Passthrough,
                likely: &[],
                extract: None,
            },
            ResolvedSpec {
                name: "calendar.look",
                description: "look",
                example: "",
                schema: None,
                aliases: &[],
                rest_field: None,
                safety: Safety::Immediate,
                respond: RespondMode::Passthrough,
                likely: &[],
                extract: None,
            },
        ]
    }

    #[test]
    fn kv_and_slash_and_json() {
        let specs = specs();
        let hit =
            parse_canonical(r#"todo.add title="Buy milk" deadline=2026-08-18"#, &specs).unwrap();
        assert_eq!(hit.tool, "todo.add");
        let v: Value = serde_json::from_str(&hit.input).unwrap();
        assert_eq!(v["title"], "Buy milk");
        assert_eq!(v["deadline"], "2026-08-18");

        let slash = parse_canonical(r#"/todo.add title="Eggs""#, &specs).unwrap();
        assert_eq!(slash.tool, "todo.add");

        let json_hit = parse_canonical(r#"{"tool":"todo.add","title":"Milk"}"#, &specs).unwrap();
        assert_eq!(json_hit.tool, "todo.add");
        let v: Value = serde_json::from_str(&json_hit.input).unwrap();
        assert_eq!(v["title"], "Milk");
        assert!(v.get("tool").is_none());
    }

    #[test]
    fn rest_field_and_empty_args() {
        let specs = specs();
        let echo = parse_canonical("echo hello there", &specs).unwrap();
        let v: Value = serde_json::from_str(&echo.input).unwrap();
        assert_eq!(v["text"], "hello there");

        let empty = parse_canonical("todo.add", &specs).unwrap();
        assert_eq!(empty.input, "{}");

        let positional = parse_canonical("todo.add Buy milk", &specs).unwrap();
        let v: Value = serde_json::from_str(&positional.input).unwrap();
        assert_eq!(v["title"], "Buy milk");
    }

    #[test]
    fn rejects_unknown_keys_and_chat() {
        let specs = specs();
        assert!(parse_canonical(r#"todo.add title="x" bogus=1"#, &specs).is_none());
        assert!(parse_canonical("whats the capital of the uk", &specs).is_none());
        assert!(parse_canonical("calendar.look what's on today?", &specs).is_none());
        let look = parse_canonical("calendar.look when=today focus=free", &specs).unwrap();
        assert_eq!(look.tool, "calendar.look");
    }

    #[test]
    fn alias_and_numbers() {
        let specs = specs();
        let hit = parse_canonical(r#"/todo title="Gym" priority=high"#, &specs).unwrap();
        let v: Value = serde_json::from_str(&hit.input).unwrap();
        assert_eq!(v["title"], "Gym");
        assert_eq!(v["priority"], "high");
    }
}
