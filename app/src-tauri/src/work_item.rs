//! One durable representation of what Buddy is trying to accomplish.
//!
//! Live turns used to split state across `PendingClarification`,
//! `NativeTranscript`, and `AgentTurn` — the last two shared `agent_turn:{cid}`
//! and could overwrite each other. New writes go to `work_item:{cid}`. Legacy
//! keys are still readable and migrated on first access.

use buddy_clarification::{JobPhase, PendingClarification};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const WORK_ITEM_SCHEMA: u32 = 1;

pub fn work_item_key(conversation_id: &str) -> String {
    format!("work_item:{conversation_id}")
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum WorkStatus {
    #[default]
    Active,
    Clarifying,
    AwaitingApproval,
    Paused,
    Done,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct CompactObservation {
    pub kind: String,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct WorkAction {
    pub id: String,
    pub tool: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct PendingApproval {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct WorkProvenance {
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub schema_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkItem {
    pub id: String,
    pub conversation_id: String,
    #[serde(default)]
    pub objective: String,
    #[serde(default)]
    pub status: WorkStatus,
    #[serde(default)]
    pub goals: Vec<String>,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub assumptions: Vec<String>,
    #[serde(default)]
    pub actions: Vec<WorkAction>,
    #[serde(default)]
    pub completed_actions: Vec<WorkAction>,
    #[serde(default)]
    pub observations: Vec<CompactObservation>,
    #[serde(default)]
    pub pending_clarification: Option<PendingClarification>,
    #[serde(default)]
    pub pending_approval: Option<PendingApproval>,
    #[serde(default)]
    pub progress: Option<String>,
    #[serde(default)]
    pub completion_condition: Option<String>,
    #[serde(default)]
    pub provenance: WorkProvenance,
    #[serde(default)]
    pub updated_at: String,
    /// Full native dialogue (messages + scratchpad). Never thinned to a scratchpad.
    #[serde(default)]
    pub transcript: Option<Value>,
}

impl WorkItem {
    pub fn new(conversation_id: &str) -> Self {
        Self {
            id: format!("work:{conversation_id}"),
            conversation_id: conversation_id.to_string(),
            provenance: WorkProvenance {
                source: "runtime".into(),
                schema_version: WORK_ITEM_SCHEMA,
            },
            updated_at: now_stamp(),
            ..Default::default()
        }
    }

    pub fn touch(&mut self) {
        self.updated_at = now_stamp();
        if self.provenance.schema_version == 0 {
            self.provenance.schema_version = WORK_ITEM_SCHEMA;
        }
    }

    pub fn is_idle(&self) -> bool {
        self.pending_clarification.is_none()
            && self.pending_approval.is_none()
            && transcript_is_empty(self.transcript.as_ref())
            && self.actions.is_empty()
    }

    pub fn set_pending(&mut self, pending: PendingClarification) {
        if self.objective.trim().is_empty() {
            self.objective = pending
                .agent_goal
                .clone()
                .filter(|g| !g.trim().is_empty())
                .unwrap_or_else(|| pending.tool.clone());
        }
        self.status = match pending.phase {
            JobPhase::NeedsInput => WorkStatus::Clarifying,
            JobPhase::AwaitingConfirm => WorkStatus::AwaitingApproval,
            JobPhase::Running => WorkStatus::Active,
        };
        self.pending_clarification = Some(pending);
        self.touch();
    }

    pub fn clear_pending(&mut self) {
        self.pending_clarification = None;
        if self.pending_approval.is_none() && self.status == WorkStatus::Clarifying {
            self.status = WorkStatus::Active;
        }
        self.touch();
    }

    pub fn set_approval(&mut self, approval: PendingApproval) {
        self.status = WorkStatus::AwaitingApproval;
        self.pending_approval = Some(approval);
        self.touch();
    }

    pub fn clear_approval(&mut self) {
        self.pending_approval = None;
        if self.pending_clarification.is_none() && self.status == WorkStatus::AwaitingApproval {
            self.status = WorkStatus::Active;
        }
        self.touch();
    }

    /// Merge an `AgentTurn` or `NativeTranscript` blob without dropping messages.
    pub fn merge_turn_blob(&mut self, raw: &str) {
        if let Some(merged) = merge_turn_blob(self.transcript.clone(), raw) {
            if self.objective.trim().is_empty() {
                if let Some(goal) = merged.get("goal").and_then(|g| g.as_str()) {
                    if !goal.trim().is_empty() {
                        self.objective = goal.to_string();
                    }
                }
            }
            self.observations = observations_from_transcript(&merged);
            self.transcript = Some(merged);
        }
        if self.status == WorkStatus::Done || self.status == WorkStatus::Cancelled {
            self.status = WorkStatus::Active;
        }
        self.touch();
    }

    pub fn clear_transcript(&mut self) {
        self.transcript = None;
        self.observations.clear();
        self.touch();
    }

    pub fn transcript_json(&self) -> Option<String> {
        self.transcript
            .as_ref()
            .and_then(|v| serde_json::to_string(v).ok())
    }
}

pub fn now_stamp() -> String {
    Utc::now().to_rfc3339()
}

pub fn transcript_is_empty(transcript: Option<&Value>) -> bool {
    let Some(v) = transcript else {
        return true;
    };
    let no_messages = v
        .get("messages")
        .and_then(|m| m.as_array())
        .map(|a| a.is_empty())
        .unwrap_or(true);
    let no_scratch = v
        .get("scratchpad")
        .and_then(|m| m.as_array())
        .map(|a| a.is_empty())
        .unwrap_or(true);
    no_messages && no_scratch
}

/// Keep native messages when a later AgentTurn (goal + scratchpad only) is saved.
pub fn merge_turn_blob(existing: Option<Value>, incoming: &str) -> Option<Value> {
    let incoming: Value = serde_json::from_str(incoming).ok()?;
    let incoming_has_messages = incoming
        .get("messages")
        .and_then(|m| m.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false);

    match existing {
        Some(mut cur) if !incoming_has_messages => {
            if let Some(goal) = incoming.get("goal") {
                cur["goal"] = goal.clone();
            }
            if let Some(scratch) = incoming.get("scratchpad") {
                cur["scratchpad"] = scratch.clone();
            }
            Some(cur)
        }
        _ => Some(incoming),
    }
}

pub fn observations_from_transcript(transcript: &Value) -> Vec<CompactObservation> {
    let Some(steps) = transcript.get("scratchpad").and_then(|s| s.as_array()) else {
        return Vec::new();
    };
    steps
        .iter()
        .map(|step| CompactObservation {
            kind: "tool".into(),
            summary: step
                .get("summary")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string(),
            payload: step
                .get("tool")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string()),
        })
        .filter(|o| !o.summary.is_empty() || o.payload.is_some())
        .collect()
}

pub fn hydrate_from_legacy(
    conversation_id: &str,
    pending: Option<PendingClarification>,
    turn_raw: Option<&str>,
) -> Option<WorkItem> {
    if pending.is_none() && turn_raw.map(|s| s.trim().is_empty()).unwrap_or(true) {
        return None;
    }
    let mut item = WorkItem::new(conversation_id);
    item.provenance.source = "legacy_migrate".into();
    if let Some(pending) = pending {
        item.set_pending(pending);
    }
    if let Some(raw) = turn_raw.filter(|s| !s.trim().is_empty()) {
        item.merge_turn_blob(raw);
    }
    Some(item)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_turn_does_not_erase_native_messages() {
        let transcript = json!({
            "goal": "plan the week",
            "messages": [{"role": "user", "content": "plan Friday"}],
            "scratchpad": [{"tool": "calendar.look", "summary": "looked"}],
        });
        let agent_turn = json!({
            "goal": "plan the week",
            "scratchpad": [{"tool": "calendar.organize", "summary": "proposed"}],
        });
        let merged = merge_turn_blob(Some(transcript), &agent_turn.to_string()).unwrap();
        assert_eq!(merged["messages"].as_array().unwrap().len(), 1);
        assert_eq!(merged["scratchpad"][0]["tool"], "calendar.organize");
    }

    #[test]
    fn native_transcript_replace_keeps_latest_messages() {
        let old = json!({"goal": "a", "messages": [{"role": "user", "content": "old"}]});
        let new = json!({"goal": "a", "messages": [
            {"role": "user", "content": "old"},
            {"role": "assistant", "content": "ok"}
        ]});
        let merged = merge_turn_blob(Some(old), &new.to_string()).unwrap();
        assert_eq!(merged["messages"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn hydrate_combines_pending_and_turn() {
        let pending = PendingClarification {
            tool: "todo.add".into(),
            tool_input: r#"{"title":"milk"}"#.into(),
            missing: vec![],
            missing_labels: vec!["deadline".into()],
            conversation_id: "c1".into(),
            follow_up: false,
            agent_scratchpad: None,
            agent_goal: Some("remind me".into()),
            phase: JobPhase::NeedsInput,
            last_proposal: None,
        };
        let turn = json!({"goal":"remind me","messages":[{"role":"user","content":"hi"}]});
        let item = hydrate_from_legacy("c1", Some(pending), Some(&turn.to_string())).unwrap();
        assert_eq!(item.conversation_id, "c1");
        assert_eq!(item.objective, "remind me");
        assert_eq!(item.status, WorkStatus::Clarifying);
        assert_eq!(item.pending_clarification.unwrap().tool, "todo.add");
        assert!(!transcript_is_empty(item.transcript.as_ref()));
    }
}
