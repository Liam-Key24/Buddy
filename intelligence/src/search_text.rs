use buddy_memory::{MemoryKind, MemoryRecord};

pub fn extract_search_text(kind: MemoryKind, payload: &serde_json::Value) -> String {
    match kind {
        MemoryKind::Working => {
            let objective = payload.get("objective").and_then(|v| v.as_str()).unwrap_or("");
            let plan = payload.get("plan").and_then(|v| v.as_str()).unwrap_or("");
            let notes = payload.get("notes").and_then(|v| v.as_str()).unwrap_or("");
            format!("{objective} {plan} {notes}")
        }
        MemoryKind::Project => {
            let section = payload.get("section").and_then(|v| v.as_str()).unwrap_or("");
            let content = payload.get("content").and_then(|v| v.as_str()).unwrap_or("");
            format!("{section}: {content}")
        }
        MemoryKind::Preference => {
            let key = payload.get("key").and_then(|v| v.as_str()).unwrap_or("");
            let value = payload.get("value").and_then(|v| v.as_str()).unwrap_or("");
            format!("{key}: {value}")
        }
        MemoryKind::Handover => {
            if let Some(summary) = payload.get("summary") {
                if let Some(s) = summary.as_str() {
                    return s.to_string();
                }
                return summary.to_string();
            }
            payload.to_string()
        }
        MemoryKind::Decision => {
            let decision = payload.get("decision").and_then(|v| v.as_str()).unwrap_or("");
            let reason = payload.get("reason").and_then(|v| v.as_str()).unwrap_or("");
            format!("{decision} {reason}")
        }
        MemoryKind::Error => {
            let error = payload.get("error").and_then(|v| v.as_str()).unwrap_or("");
            let cause = payload.get("cause").and_then(|v| v.as_str()).unwrap_or("");
            let resolution = payload.get("resolution").and_then(|v| v.as_str()).unwrap_or("");
            format!("{error} {cause} {resolution}")
        }
        MemoryKind::Tool => {
            let tool = payload.get("tool").and_then(|v| v.as_str()).unwrap_or("");
            let params = payload.get("params").and_then(|v| v.as_str()).unwrap_or("");
            let result = payload.get("result").and_then(|v| v.as_str()).unwrap_or("");
            format!("{tool} {params} {result}")
        }
        MemoryKind::Reflection => {
            let attempted = payload.get("attempted").and_then(|v| v.as_str()).unwrap_or("");
            let improvements = payload.get("improvements").and_then(|v| v.as_str()).unwrap_or("");
            let lessons = payload.get("lessons").and_then(|v| v.as_str()).unwrap_or("");
            format!("{attempted} {improvements} {lessons}")
        }
        MemoryKind::Conversation => payload
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    }
    .trim()
    .to_string()
}

pub fn extract_confidence(payload: &serde_json::Value) -> Option<f64> {
    payload.get("confidence").and_then(|v| v.as_f64())
}

pub fn payload_from_record(record: &MemoryRecord) -> &serde_json::Value {
    &record.payload
}
