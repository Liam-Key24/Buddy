//! Deterministic ViewModel for known tool results. No extra model call.

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Fact {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ToolViewModel {
    pub headline: String,
    pub facts: Vec<Fact>,
    pub items: Vec<String>,
    pub needs_approval: bool,
}

impl ToolViewModel {
    pub fn render(&self) -> String {
        let mut lines = vec![self.headline.clone()];
        for fact in &self.facts {
            if fact.value.is_empty() {
                lines.push(fact.label.clone());
            } else {
                lines.push(format!("{}: {}", fact.label, fact.value));
            }
        }
        for item in &self.items {
            lines.push(format!("- {item}"));
        }
        if self.needs_approval {
            lines.push("Nothing is on the calendar until you approve.".into());
        }
        lines.join("\n")
    }
}

pub fn view_model_from_json(tool: &str, value: &Value) -> Option<ToolViewModel> {
    if tool == "goal.intake" {
        let titles: Vec<String> = value
            .get("goals")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|g| g.get("title")?.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        let questions: Vec<String> = value
            .get("questions")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|q| q.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        return Some(ToolViewModel {
            headline: if titles.len() > 1 {
                format!("Captured {} goals.", titles.len())
            } else {
                format!("Captured {}.", titles.first().map(String::as_str).unwrap_or("a goal"))
            },
            facts: Vec::new(),
            items: questions,
            needs_approval: false,
        });
    }
    if tool == "goal.propose_plan" {
        let requested = value
            .get("weekly_minutes_requested")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let assumptions: Vec<String> = value
            .get("assumptions")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|a| a.as_str().map(|s| s.to_string()))
                    .take(3)
                    .collect()
            })
            .unwrap_or_default();
        return Some(ToolViewModel {
            headline: format!("Draft week uses {requested} minutes."),
            facts: Vec::new(),
            items: assumptions,
            needs_approval: value
                .get("needs_approval")
                .and_then(|v| v.as_bool())
                .unwrap_or(true),
        });
    }
    if tool == "goal.record_progress" {
        let forecast = value
            .get("forecast")
            .and_then(|v| v.as_str())
            .unwrap_or("on_track");
        let released = value
            .get("released_sessions")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let options: Vec<String> = value
            .get("adapt_options")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|o| o.get("summary")?.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        return Some(ToolViewModel {
            headline: format!("Progress updated. Forecast: {}.", forecast.replace('_', " ")),
            facts: if released > 0 {
                vec![Fact {
                    label: "Released sessions".into(),
                    value: format!("{released} (left free)"),
                }]
            } else {
                vec![]
            },
            items: options,
            needs_approval: true,
        });
    }
    if tool == "goal.look" {
        let goals = value.get("goals").and_then(|v| v.as_array())?;
        let items: Vec<String> = goals
            .iter()
            .filter_map(|g| {
                Some(format!(
                    "{} — {}",
                    g.get("title")?.as_str()?,
                    g.get("forecast")?.as_str()?.replace('_', " ")
                ))
            })
            .collect();
        return Some(ToolViewModel {
            headline: if items.is_empty() {
                "No active goals.".into()
            } else {
                format!("{} active goals.", items.len())
            },
            facts: Vec::new(),
            items,
            needs_approval: false,
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn goal_plan_view_is_deterministic() {
        let vm = view_model_from_json(
            "goal.propose_plan",
            &json!({
                "weekly_minutes_requested": 420,
                "needs_approval": true,
                "assumptions": ["Released time stays free unless you approve a reallocation."]
            }),
        )
        .unwrap();
        let text = vm.render();
        assert!(text.contains("420"));
        assert!(text.contains("approve"));
        assert!(vm.needs_approval);
    }
}
