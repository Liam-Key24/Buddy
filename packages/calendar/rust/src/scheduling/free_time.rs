use crate::scheduling::occupancy::{build_occupancy_excluding, merged_hard_blocks};
use crate::scheduling::score::score_slot_for_activity;
use crate::scheduling::types::FreeSlot;
use crate::scheduling::SchedulingContext;

/// Find free slots of at least `duration_ms` within the context range.
pub fn find_free_slots(
    ctx: &SchedulingContext,
    duration_ms: i64,
    limit: usize,
    exclude_event_id: Option<&str>,
) -> Vec<FreeSlot> {
    find_free_slots_for(ctx, duration_ms, limit, exclude_event_id, None)
}

/// Like [`find_free_slots`], with activity-aware ranking (e.g. dinner → evening).
pub fn find_free_slots_for(
    ctx: &SchedulingContext,
    duration_ms: i64,
    limit: usize,
    exclude_event_id: Option<&str>,
    activity_title: Option<&str>,
) -> Vec<FreeSlot> {
    if duration_ms <= 0 || ctx.range.end <= ctx.range.start {
        return Vec::new();
    }

    let occupancy = build_occupancy_excluding(ctx, exclude_event_id);
    let hard = merged_hard_blocks(&occupancy, &ctx.policy);

    let mut gaps = Vec::new();
    let mut cursor = ctx.range.start;
    for (s, e) in &hard {
        if *s > cursor {
            gaps.push((cursor, *s));
        }
        cursor = cursor.max(*e);
    }
    if cursor < ctx.range.end {
        gaps.push((cursor, ctx.range.end));
    }

    let mut slots = Vec::new();
    for (gap_start, gap_end) in gaps {
        let mut start = gap_start;
        while start + duration_ms <= gap_end {
            let end = start + duration_ms;
            let (score, reasons) =
                score_slot_for_activity(ctx, start, end, &occupancy, activity_title);
            slots.push(FreeSlot {
                start,
                end,
                score,
                reasons,
            });
            start += 15 * 60_000;
            if slots.len() > 500 {
                break;
            }
        }
        // Also consider placing at the end of the gap if it fits.
        if gap_end - gap_start >= duration_ms {
            let end = gap_end;
            let start = end - duration_ms;
            if start >= gap_start && !slots.iter().any(|s| s.start == start && s.end == end) {
                let (score, reasons) =
                    score_slot_for_activity(ctx, start, end, &occupancy, activity_title);
                slots.push(FreeSlot {
                    start,
                    end,
                    score,
                    reasons,
                });
            }
        }
    }

    slots.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.start.cmp(&b.start))
    });

    diversify_slots(&slots, limit.max(1))
}

fn diversify_slots(sorted: &[FreeSlot], limit: usize) -> Vec<FreeSlot> {
    let mut out = Vec::new();
    for slot in sorted {
        let overlaps_picked = out.iter().any(|p: &FreeSlot| {
            let overlap = slot.start < p.end && slot.end > p.start;
            if !overlap {
                return false;
            }
            let overlap_ms = (slot.end.min(p.end) - slot.start.max(p.start)).max(0);
            let shorter = (slot.end - slot.start).min(p.end - p.start).max(1);
            overlap_ms * 100 / shorter >= 50
        });
        if !overlaps_picked {
            out.push(slot.clone());
        }
        if out.len() >= limit {
            break;
        }
    }
    out
}
