//! Cheap turn router: canonical syntax / extract → tool jobs, else miss.

use serde_json::{json, Value};

use crate::spec::{ResolvedSpec, Safety};
use crate::syntax::{parse_canonical, CanonicalHit};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolJob {
    pub tool: String,
    pub input: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    /// Structured or high-confidence extract. Clarification still gates execute.
    Tools(Vec<ToolJob>),
    /// Not canonical — orchestrator continues with NL / chat paths.
    Miss,
}

pub fn route(text: &str, specs: &[ResolvedSpec]) -> Route {
    if let Some(hit) = parse_canonical(text, specs) {
        return Route::Tools(vec![job_from_hit(hit, specs)]);
    }
    let mut jobs = Vec::new();
    for spec in specs {
        if let Some(extract) = spec.extract {
            if let Some(input) = extract(text) {
                jobs.push(apply_safety(
                    spec,
                    ToolJob {
                        tool: spec.name.to_string(),
                        input,
                    },
                ));
            }
        }
    }
    if jobs.is_empty() {
        Route::Miss
    } else {
        Route::Tools(jobs)
    }
}

fn job_from_hit(hit: CanonicalHit, specs: &[ResolvedSpec]) -> ToolJob {
    let spec = specs.iter().find(|s| s.name == hit.tool);
    let job = ToolJob {
        tool: hit.tool.to_string(),
        input: hit.input,
    };
    match spec {
        Some(spec) => apply_safety(spec, job),
        None => job,
    }
}

fn apply_safety(spec: &ResolvedSpec, mut job: ToolJob) -> ToolJob {
    if spec.safety != Safety::ProposeFirst {
        return job;
    }
    let mut value: Value = serde_json::from_str(&job.input).unwrap_or_else(|_| json!({}));
    if let Some(obj) = value.as_object_mut() {
        let has_mode = obj.get("mode").and_then(|v| v.as_str()).is_some();
        let apply = obj.get("apply").and_then(|v| v.as_bool()).unwrap_or(false);
        if !has_mode && !apply {
            obj.insert("mode".into(), json!("propose"));
        }
    }
    job.input = value.to_string();
    job
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{AskKind, FieldSpec, ToolSchema};
    use crate::spec::RespondMode;

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

    fn extract_todo(text: &str) -> Option<String> {
        let lower = text.to_ascii_lowercase();
        let idx = lower.find("remind me to ")? + 13;
        let title = text.get(idx..)?.trim();
        if title.len() < 2 {
            return None;
        }
        Some(json!({ "title": title }).to_string())
    }

    fn specs() -> Vec<ResolvedSpec> {
        vec![
            ResolvedSpec {
                name: "todo.add",
                description: "add",
                example: "",
                schema: Some(&SCHEMA),
                aliases: &[],
                rest_field: Some("title"),
                safety: Safety::Immediate,
                respond: RespondMode::Passthrough,
                likely: &["remind me to"],
                extract: Some(extract_todo),
            },
            ResolvedSpec {
                name: "calendar.organize",
                description: "organize",
                example: "",
                schema: None,
                aliases: &[],
                rest_field: None,
                safety: Safety::ProposeFirst,
                respond: RespondMode::Passthrough,
                likely: &[],
                extract: None,
            },
        ]
    }

    #[test]
    fn canonical_hits_and_chat_misses() {
        let specs = specs();
        match route(r#"todo.add title="Milk""#, &specs) {
            Route::Tools(jobs) => {
                assert_eq!(jobs[0].tool, "todo.add");
                assert!(jobs[0].input.contains("Milk"));
            }
            Route::Miss => panic!("expected hit"),
        }
        assert_eq!(route("whats the capital of the uk", &specs), Route::Miss);
    }

    #[test]
    fn extract_nl_without_syntax() {
        let specs = specs();
        match route("remind me to buy oat milk", &specs) {
            Route::Tools(jobs) => {
                assert_eq!(jobs[0].tool, "todo.add");
                let v: Value = serde_json::from_str(&jobs[0].input).unwrap();
                assert_eq!(v["title"], "buy oat milk");
            }
            Route::Miss => panic!("expected extract"),
        }
    }

    #[test]
    fn propose_first_fills_mode() {
        let specs = specs();
        match route(r#"calendar.organize window=this_week"#, &specs) {
            Route::Tools(jobs) => {
                let v: Value = serde_json::from_str(&jobs[0].input).unwrap();
                assert_eq!(v["mode"], "propose");
                assert_eq!(v["window"], "this_week");
            }
            Route::Miss => panic!("expected organize"),
        }
    }
}
