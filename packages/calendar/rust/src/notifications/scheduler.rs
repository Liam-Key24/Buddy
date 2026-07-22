use buddy_database::{chrono_now, Database, ReminderStateRow};
use uuid::Uuid;

use crate::error::CalendarError;
use crate::models::{Event, Reminder, ReminderDelivery};

/// Rebuild pending reminder states for an event from its reminder list.
pub fn rebuild_reminders_for_event(
    db: &Database,
    event: &Event,
) -> Result<(), CalendarError> {
    db.delete_reminder_states_for_event(&event.id)?;
    let now = chrono_now();
    for rem in &event.reminders {
        let fire_at = event.start_time - (rem.minutes_before as i64) * 60_000;
        // Skip reminders that already fired in the past for past events
        if fire_at < now && event.start_time < now {
            continue;
        }
        let row = ReminderStateRow {
            id: Uuid::new_v4().to_string(),
            event_id: event.id.clone(),
            reminder_minutes: rem.minutes_before as i64,
            fire_at,
            status: "pending".into(),
            snoozed_until: None,
            delivered_at: None,
            created_at: now,
        };
        db.upsert_reminder_state(&row)?;
    }
    Ok(())
}

pub fn list_due_deliveries(db: &Database, now_ms: i64) -> Result<Vec<ReminderDelivery>, CalendarError> {
    let rows = db.list_due_reminders(now_ms)?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let title = db
            .get_buddy_calendar_event(&row.event_id)
            .map(|e| e.title)
            .unwrap_or_else(|_| "Event".into());
        out.push(ReminderDelivery {
            id: row.id,
            event_id: row.event_id,
            event_title: title,
            reminder_minutes: row.reminder_minutes,
            fire_at: row.fire_at,
            status: row.status,
            snoozed_until: row.snoozed_until,
            delivered_at: row.delivered_at,
        });
    }
    Ok(out)
}

pub fn mark_reminder_sent(db: &Database, id: &str) -> Result<(), CalendarError> {
    db.update_reminder_status(id, "sent", None, Some(chrono_now()))?;
    Ok(())
}

pub fn snooze_reminder(
    db: &Database,
    id: &str,
    minutes: u32,
) -> Result<(), CalendarError> {
    let until = chrono_now() + (minutes as i64) * 60_000;
    db.snooze_reminder(id, until)?;
    Ok(())
}

pub fn dismiss_reminder(db: &Database, id: &str) -> Result<(), CalendarError> {
    db.dismiss_reminder(id)?;
    Ok(())
}

pub fn list_notifications(db: &Database) -> Result<Vec<ReminderDelivery>, CalendarError> {
    let rows = db.list_active_notifications()?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let title = db
            .get_buddy_calendar_event(&row.event_id)
            .map(|e| e.title)
            .unwrap_or_else(|_| "Event".into());
        out.push(ReminderDelivery {
            id: row.id,
            event_id: row.event_id,
            event_title: title,
            reminder_minutes: row.reminder_minutes,
            fire_at: row.fire_at,
            status: row.status,
            snoozed_until: row.snoozed_until,
            delivered_at: row.delivered_at,
        });
    }
    Ok(out)
}

pub fn parse_reminders(json: &str) -> Vec<Reminder> {
    serde_json::from_str(json).unwrap_or_default()
}

pub fn serialize_reminders(reminders: &[Reminder]) -> String {
    serde_json::to_string(reminders).unwrap_or_else(|_| "[]".into())
}
