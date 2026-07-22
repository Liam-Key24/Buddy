use serde::{Deserialize, Serialize};

use crate::models::ScheduleKind;
use crate::scheduling::types::SchedulingPolicy;
use crate::scheduling::SchedulingContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BusySource {
    Event,
    Sleep,
    Work,
    Buffer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusyInterval {
    pub start: i64,
    pub end: i64,
    pub source: BusySource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<String>,
    /// When source is Buffer, the event that owns the buffer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_event_id: Option<String>,
}

impl BusyInterval {
    pub fn overlaps(&self, start: i64, end: i64) -> bool {
        self.start < end && self.end > start
    }

    pub fn is_protected(&self) -> bool {
        matches!(self.source, BusySource::Sleep | BusySource::Work)
    }
}

/// Build sorted busy intervals (events + lifestyle + buffers).
pub fn build_occupancy(ctx: &SchedulingContext) -> Vec<BusyInterval> {
    build_occupancy_excluding(ctx, None)
}

/// Like [`build_occupancy`] but skips a specific event id (for update checks).
pub fn build_occupancy_excluding(
    ctx: &SchedulingContext,
    exclude_event_id: Option<&str>,
) -> Vec<BusyInterval> {
    let mut intervals = Vec::new();

    for block in &ctx.lifestyle_blocks {
        if block.end_time <= ctx.range.start || block.start_time >= ctx.range.end {
            continue;
        }
        let source = match block.kind {
            ScheduleKind::Sleep => {
                if !ctx.policy.protect_sleep {
                    continue;
                }
                BusySource::Sleep
            }
            ScheduleKind::Work => {
                if !ctx.policy.protect_work {
                    continue;
                }
                BusySource::Work
            }
        };
        intervals.push(BusyInterval {
            start: block.start_time.max(ctx.range.start),
            end: block.end_time.min(ctx.range.end),
            source,
            label: Some(block.title.clone()),
            event_id: Some(block.id.clone()),
            related_event_id: None,
        });
    }

    let buffer_ms = ctx.policy.buffer_ms();
    for event in &ctx.events {
        if let Some(ex) = exclude_event_id {
            let master = event.id.split("::").next().unwrap_or(event.id.as_str());
            let ex_master = ex.split("::").next().unwrap_or(ex);
            if master == ex_master || event.id == ex {
                continue;
            }
        }
        if event.end_time <= ctx.range.start || event.start_time >= ctx.range.end {
            continue;
        }
        intervals.push(BusyInterval {
            start: event.start_time,
            end: event.end_time,
            source: BusySource::Event,
            label: Some(event.title.clone()),
            event_id: Some(event.id.clone()),
            related_event_id: None,
        });

        if !event.all_day && buffer_ms > 0 {
            let before_start = event.start_time.saturating_sub(buffer_ms);
            if before_start < event.start_time {
                intervals.push(BusyInterval {
                    start: before_start,
                    end: event.start_time,
                    source: BusySource::Buffer,
                    label: Some(format!("Buffer before {}", event.title)),
                    event_id: None,
                    related_event_id: Some(event.id.clone()),
                });
            }
            let after_end = event.end_time.saturating_add(buffer_ms);
            if after_end > event.end_time {
                intervals.push(BusyInterval {
                    start: event.end_time,
                    end: after_end,
                    source: BusySource::Buffer,
                    label: Some(format!("Buffer after {}", event.title)),
                    event_id: None,
                    related_event_id: Some(event.id.clone()),
                });
            }
        }
    }

    intervals.sort_by_key(|i| (i.start, i.end));
    merge_adjacent_same_source(&mut intervals);
    intervals
}

fn merge_adjacent_same_source(intervals: &mut Vec<BusyInterval>) {
    if intervals.is_empty() {
        return;
    }
    let mut merged = Vec::with_capacity(intervals.len());
    let mut current = intervals[0].clone();
    for next in intervals.iter().skip(1) {
        if next.source == current.source
            && next.start <= current.end
            && next.event_id == current.event_id
            && next.related_event_id == current.related_event_id
        {
            current.end = current.end.max(next.end);
        } else {
            merged.push(current);
            current = next.clone();
        }
    }
    merged.push(current);
    *intervals = merged;
}

/// Merge all busy intervals into a sorted list of hard blocks for gap finding.
pub fn merged_hard_blocks(
    intervals: &[BusyInterval],
    policy: &SchedulingPolicy,
) -> Vec<(i64, i64)> {
    let mut blocks: Vec<(i64, i64)> = intervals
        .iter()
        .filter(|i| {
            if i.source == BusySource::Buffer && policy.allow_reduce_buffer {
                return false;
            }
            true
        })
        .map(|i| (i.start, i.end))
        .collect();
    if blocks.is_empty() {
        return blocks;
    }
    blocks.sort_by_key(|b| b.0);
    let mut merged = vec![blocks[0]];
    for (s, e) in blocks.into_iter().skip(1) {
        let last = merged.last_mut().unwrap();
        if s <= last.1 {
            last.1 = last.1.max(e);
        } else {
            merged.push((s, e));
        }
    }
    merged
}


