//! Canonical tool contract: one spec per tool for syntax, schema, and routing.
//!
//! Plugins declare [`ToolSpec`]. Catalog / OpenAI schemas / the router all derive
//! from it (or from a [`ResolvedSpec`] synthesized from older `ToolDecl` + `ToolSchema`).

use crate::plugin::ToolDecl;
use crate::schema::ToolSchema;

/// Whether a parsed call may run immediately or must stay in a safe mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Safety {
    #[default]
    Immediate,
    /// Fill `mode=propose` when the user did not confirm.
    ProposeFirst,
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
    pub respond: RespondMode,
    /// Cheap NL phrases that mean this tool is likely (used by extract / infer).
    pub likely: &'static [&'static str],
    /// High-confidence NL → canonical JSON `tool_input`. None = syntax only.
    pub extract: Option<fn(&str) -> Option<String>>,
}

impl ToolSpec {
    pub fn planner_line(&self) -> String {
        if self.example.is_empty() {
            format!("{}: {}", self.name, self.description)
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
    pub respond: RespondMode,
    pub likely: &'static [&'static str],
    pub extract: Option<fn(&str) -> Option<String>>,
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
            respond: spec.respond,
            likely: spec.likely,
            extract: spec.extract,
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
            respond: RespondMode::Passthrough,
            likely: &[],
            extract: None,
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
}

fn default_safety(name: &str) -> Safety {
    if name == "calendar.organize" {
        Safety::ProposeFirst
    } else {
        Safety::Immediate
    }
}
