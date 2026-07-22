use serde::{Deserialize, Serialize};

use crate::scheduling::free_time::find_free_slots;
use crate::scheduling::occupancy::{build_occupancy_excluding, BusyInterval, BusySource};
use crate::scheduling::types::{Suggestion, SuggestionAction};
use crate::scheduling::SchedulingContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictKind {
    Overlap,
    BufferViolation,
    ProtectedSleep,
    ProtectedWork,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictDetail {
    pub kind: ConflictKind,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflicting_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflicting_label: Option<String>,
    pub start: i64,
    pub end: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictReport {
    pub has_conflicts: bool,
    pub conflicts: Vec<ConflictDetail>,
    pub suggestions: Vec<Suggestion>,
}

impl ConflictReport {
    pub fn clean() -> Self {
        Self {
            has_conflicts: false,
            conflicts: Vec::new(),
            suggestions: Vec::new(),
        }
    }
}

/// Detect conflicts for a proposed window `[start, end)`.
pub fn detect_conflicts(
    ctx: &SchedulingContext,
    start: i64,
    end: i64,
    exclude_event_id: Option<&str>,
) -> ConflictReport {
    let occupancy = build_occupancy_excluding(ctx, exclude_event_id);
    let mut conflicts = Vec::new();

    for b in &occupancy {
        if !b.overlaps(start, end) {
            continue;
        }
        match b.source {
            BusySource::Sleep => conflicts.push(detail(
                ConflictKind::ProtectedSleep,
                "Overlaps a protected sleep block",
                b,
            )),
            BusySource::Work => conflicts.push(detail(
                ConflictKind::ProtectedWork,
                "Overlaps a protected work block",
                b,
            )),
            BusySource::Event => conflicts.push(detail(
                ConflictKind::Overlap,
                &format!(
                    "Overlaps with {}",
                    b.label.as_deref().unwrap_or("another event")
                ),
                b,
            )),
            BusySource::Buffer => {
                if !ctx.policy.allow_reduce_buffer {
                    conflicts.push(detail(
                        ConflictKind::BufferViolation,
                        "Violates buffer time between events",
                        b,
                    ));
                }
            }
        }
    }

    if conflicts.is_empty() {
        return ConflictReport::clean();
    }

    let duration = end - start;
    let mut suggestions = Vec::new();
    suggestions.push(Suggestion {
        action: SuggestionAction::KeepExisting,
        message: "Keep the existing schedule and choose another time for the new event.".into(),
        event_id: None,
        start: None,
        end: None,
    });
    suggestions.push(Suggestion {
        action: SuggestionAction::MoveNew,
        message: "Move the new event to a free slot that respects sleep, work, and buffers.".into(),
        event_id: None,
        start: None,
        end: None,
    });

    let alts = find_free_slots(ctx, duration, 3, exclude_event_id);
    for slot in alts {
        suggestions.push(Suggestion {
            action: SuggestionAction::UseSlot,
            message: format!(
                "Use alternate slot scored {:.0}: {} – {}",
                slot.score,
                format_ms(slot.start),
                format_ms(slot.end)
            ),
            event_id: None,
            start: Some(slot.start),
            end: Some(slot.end),
        });
    }

    // If only buffer violations, note that buffer can be reduced only if user asks.
    if conflicts
        .iter()
        .all(|c| c.kind == ConflictKind::BufferViolation)
    {
        suggestions.push(Suggestion {
            action: SuggestionAction::MoveNew,
            message: "Buffer time is protected. Search another slot before reducing the buffer."
                .into(),
            event_id: None,
            start: None,
            end: None,
        });
    }

    ConflictReport {
        has_conflicts: true,
        conflicts,
        suggestions,
    }
}

fn detail(kind: ConflictKind, message: &str, b: &BusyInterval) -> ConflictDetail {
    ConflictDetail {
        kind,
        message: message.into(),
        conflicting_id: b.event_id.clone().or_else(|| b.related_event_id.clone()),
        conflicting_label: b.label.clone(),
        start: b.start,
        end: b.end,
    }
}

fn format_ms(ms: i64) -> String {
    use chrono::{TimeZone, Utc};
    Utc.timestamp_millis_opt(ms)
        .single()
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| ms.to_string())
}
