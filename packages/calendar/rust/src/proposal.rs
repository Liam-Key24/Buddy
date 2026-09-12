//! Organize proposal snapshot (ghost week) parsed from tool I/O.

use serde::{Deserialize, Serialize};

use crate::scheduling::OrganizeItemIn;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GhostBlock {
    pub title: String,
    pub start: i64,
    pub end: i64,
    #[serde(default)]
    pub score: f64,
    #[serde(default)]
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StoredCalendarProposal {
    pub conversation_id: String,
    #[serde(default)]
    pub window: Option<String>,
    #[serde(default)]
    pub items: Vec<OrganizeItemIn>,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub blocks: Vec<GhostBlock>,
}

pub fn proposal_from_organize(
    conversation_id: &str,
    tool_input: &str,
    output: &str,
) -> Option<StoredCalendarProposal> {
    let input: serde_json::Value = serde_json::from_str(tool_input).ok()?;
    let out: serde_json::Value = serde_json::from_str(output).ok()?;
    let status = out.get("status").and_then(|s| s.as_str()).unwrap_or("");
    if status != "proposed" && input.get("mode").and_then(|m| m.as_str()) != Some("propose") {
        if out.get("apply").and_then(|a| a.as_bool()) != Some(false) {
            return None;
        }
    }
    let scheduled = out.get("scheduled").and_then(|v| v.as_array())?;
    if scheduled.is_empty() {
        return None;
    }
    let blocks: Vec<GhostBlock> = scheduled
        .iter()
        .filter_map(|b| {
            Some(GhostBlock {
                title: b.get("title")?.as_str()?.to_string(),
                start: b.get("start")?.as_i64()?,
                end: b.get("end")?.as_i64()?,
                score: b.get("score").and_then(|s| s.as_f64()).unwrap_or(0.0),
                reasons: b
                    .get("reasons")
                    .and_then(|r| r.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default(),
            })
        })
        .collect();
    if blocks.is_empty() {
        return None;
    }
    let items: Vec<OrganizeItemIn> = input
        .get("items")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .or_else(|| {
            Some(
                blocks
                    .iter()
                    .map(|b| OrganizeItemIn {
                        title: b.title.clone(),
                        duration: None,
                        duration_minutes: Some((((b.end - b.start).max(60_000)) / 60_000) as u32),
                        when: None,
                        count: Some(1),
                    })
                    .collect(),
            )
        })
        .unwrap_or_default();
    let constraints: Vec<String> = input
        .get("constraints")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    Some(StoredCalendarProposal {
        conversation_id: conversation_id.to_string(),
        window: input
            .get("window")
            .and_then(|w| w.as_str())
            .map(|s| s.to_string()),
        items,
        constraints,
        blocks,
    })
}
