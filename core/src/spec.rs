//! Canonical tool contract: one spec per tool for syntax, schema, and routing.
//!
//! Plugins declare [`ToolSpec`]. Catalog, OpenAI schemas, and the router all
//! derive from it (or from a [`ResolvedSpec`] synthesized from older `ToolDecl`).

use crate::plugin::ToolDecl;
use crate::schema::ToolSchema;
use serde_json::{json, Value};

/// Whether a parsed call may run immediately or must stay in a safe mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Safety {
    #[default]
    Immediate,
    /// Fill `mode=propose` when the user did not confirm.
    ProposeFirst,
}

/// Who must approve before the tool may take effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Permission {
    #[default]
    None,
    Confirm,
    External,
    Destructive,
}

/// How the orchestrator should present a successful result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RespondMode {
    #[default]
    Passthrough,
    Llm,
}

/// Plugin-facing declaration. Prefer this over a bare [`ToolDecl`] for new tools.
#[derive(Debug, Clone, Copy)]
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    /// Compact example, e.g. `todo.add title="Buy milk" deadline=2026-08-18`.
    pub example: &'static str,
    pub schema: ToolSchema,
    /// Extra names the canonical parser accepts (`/todo.add` is always allowed).
    pub aliases: &'static [&'static str],
    /// If set, leftover positional text becomes this JSON field (`echo hello`).
    pub rest_field: Option<&'static str>,
    pub safety: Safety,
    pub permission: Permission,
    pub respond: RespondMode,
    /// Cheap NL phrases that mean this tool is likely (used by extract / infer).
    pub likely: &'static [&'static str],
    /// High-confidence NL → canonical JSON `tool_input`. None = syntax only.
    pub extract: Option<fn(&str) -> Option<String>>,
    /// Optional JSON object merged into the OpenAI parameters properties map.
    pub openai_properties_json: &'static str,
}

impl ToolSpec {
    pub const fn basic(
        name: &'static str,
        description: &'static str,
        example: &'static str,
        schema: ToolSchema,
    ) -> Self {
        Self {
            name,
            description,
            example,
            schema,
            aliases: &[],
            rest_field: None,
            safety: Safety::Immediate,
            permission: Permission::None,
            respond: RespondMode::Passthrough,
            likely: &[],
            extract: None,
            openai_properties_json: "",
        }
    }

    pub fn planner_line(&self) -> String {
        if self.example.is_empty() {
            if self.description.contains(':') {
                self.description.to_string()
            } else {
                format!("{}: {}", self.name, self.description)
            }
        } else {
            format!(
                "{}: {}. Canonical: {}",
                self.name, self.description, self.example
            )
        }
    }
}

/// Router-facing view. Static plugin specs plus decls synthesized at load.
#[derive(Debug, Clone, Copy)]
pub struct ResolvedSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub example: &'static str,
    pub schema: Option<&'static ToolSchema>,
    pub aliases: &'static [&'static str],
    pub rest_field: Option<&'static str>,
    pub safety: Safety,
    pub permission: Permission,
    pub respond: RespondMode,
    pub likely: &'static [&'static str],
    pub extract: Option<fn(&str) -> Option<String>>,
    pub openai_properties_json: &'static str,
}

impl ResolvedSpec {
    pub fn from_spec(spec: &'static ToolSpec) -> Self {
        Self {
            name: spec.name,
            description: spec.description,
            example: spec.example,
            schema: Some(&spec.schema),
            aliases: spec.aliases,
            rest_field: spec.rest_field,
            safety: spec.safety,
            permission: spec.permission,
            respond: spec.respond,
            likely: spec.likely,
            extract: spec.extract,
            openai_properties_json: spec.openai_properties_json,
        }
    }

    pub fn from_decl(decl: ToolDecl, schema: Option<&'static ToolSchema>) -> Self {
        Self {
            name: decl.name,
            description: decl.planner_line,
            example: "",
            schema,
            aliases: &[],
            rest_field: None,
            safety: default_safety(decl.name),
            permission: default_permission(decl.name),
            respond: RespondMode::Passthrough,
            likely: &[],
            extract: None,
            openai_properties_json: "",
        }
    }

    pub fn family(&self) -> &str {
        self.name.split('.').next().unwrap_or(self.name)
    }

    pub fn matches_name(&self, token: &str) -> bool {
        let token = token.strip_prefix('/').unwrap_or(token);
        if self.name.eq_ignore_ascii_case(token) {
            return true;
        }
        self.aliases.iter().any(|alias| {
            let alias = alias.strip_prefix('/').unwrap_or(*alias);
            alias.eq_ignore_ascii_case(token)
        })
    }

    pub fn planner_line(&self) -> String {
        if self.example.is_empty() {
            if self.description.contains(':') {
                self.description.to_string()
            } else {
                format!("{}: {}", self.name, self.description)
            }
        } else {
            format!(
                "{}: {}. Canonical: {}",
                self.name, self.description, self.example
            )
        }
    }

    pub fn openai_tool(&self) -> Value {
        let mut properties = serde_json::Map::new();
        if let Some(schema) = self.schema {
            for field in schema.fields {
                properties.insert(
                    field.name.to_string(),
                    json!({
                        "type": "string",
                        "description": field.label,
                    }),
                );
            }
        }
        if !self.openai_properties_json.is_empty() {
            if let Ok(Value::Object(extra)) = serde_json::from_str(self.openai_properties_json) {
                for (key, value) in extra {
                    properties.insert(key, value);
                }
            }
        }
        json!({
            "type": "function",
            "function": {
                "name": self.name,
                "description": self.description,
                "parameters": {
                    "type": "object",
                    "properties": properties,
                    "additionalProperties": true
                }
            }
        })
    }
}

fn default_safety(name: &str) -> Safety {
    if name == "calendar.organize" {
        Safety::ProposeFirst
    } else {
        Safety::Immediate
    }
}

fn default_permission(name: &str) -> Permission {
    if name.starts_with("send_email") || name.starts_with("git_push") {
        Permission::External
    } else if name.contains("delete") {
        Permission::Destructive
    } else {
        Permission::None
    }
}

/// Empty clarification schema when a tool has no required fields.
pub const fn empty_schema(tool: &'static str) -> ToolSchema {
    ToolSchema { tool, fields: &[] }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{AskKind, FieldSpec};

    const FIELDS: &[FieldSpec] = &[FieldSpec {
        name: "title",
        label: "task",
        required: true,
        memory_keys: &[],
        ask_kind: AskKind::Text,
        choices: &[],
    }];
    const SCHEMA: ToolSchema = ToolSchema {
        tool: "todo.add",
        fields: FIELDS,
    };
    const SPEC: ToolSpec = ToolSpec {
        name: "todo.add",
        description: "create a task",
        example: r#"todo.add title="Milk""#,
        schema: SCHEMA,
        aliases: &[],
        rest_field: Some("title"),
        safety: Safety::Immediate,
        permission: Permission::None,
        respond: RespondMode::Passthrough,
        likely: &[],
        extract: None,
        openai_properties_json: r#"{"priority":{"type":"string"}}"#,
    };

    #[test]
    fn openai_tool_merges_schema_and_extra_properties() {
        let tool = ResolvedSpec::from_spec(&SPEC).openai_tool();
        let props = tool["function"]["parameters"]["properties"].as_object().unwrap();
        assert!(props.contains_key("title"));
        assert!(props.contains_key("priority"));
        assert_eq!(tool["function"]["name"], "todo.add");
    }

    #[test]
    fn catalog_skips_duplicate_decl_when_spec_exists() {
        let line = SPEC.planner_line();
        assert!(line.contains("todo.add"));
        assert!(line.contains("Milk"));
    }
}
