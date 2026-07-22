use buddy_database::{chrono_now, DreamEntryRow, Database};
use uuid::Uuid;

use crate::error::CalendarError;
use crate::models::{CreateDreamInput, DreamEntry, UpdateDreamInput};
use crate::services::schedule_service::last_sleep_date;

fn tags_to_json(tags: &[String]) -> String {
    serde_json::to_string(tags).unwrap_or_else(|_| "[]".into())
}

fn tags_from_json(s: &str) -> Vec<String> {
    serde_json::from_str(s).unwrap_or_default()
}

fn row_to_dream(row: DreamEntryRow) -> DreamEntry {
    DreamEntry {
        id: row.id,
        sleep_date: row.sleep_date,
        title: row.title,
        body: row.body,
        tags: tags_from_json(&row.tags_json),
        mood: row.mood,
        sleep_quality: row.sleep_quality,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

pub fn log_dream(db: &Database, input: CreateDreamInput) -> Result<DreamEntry, CalendarError> {
    let body = input.body.trim();
    if body.is_empty() {
        return Err(CalendarError::InvalidInput("dream body required".into()));
    }
    let sleep_date = match input.sleep_date {
        Some(d) if !d.trim().is_empty() => d.trim().to_string(),
        _ => last_sleep_date(db, None)?,
    };
    let now = chrono_now();
    let row = DreamEntryRow {
        id: Uuid::new_v4().to_string(),
        sleep_date,
        title: input.title.filter(|t| !t.trim().is_empty()),
        body: body.to_string(),
        tags_json: tags_to_json(&input.tags),
        mood: input.mood,
        sleep_quality: input.sleep_quality,
        created_at: now,
        updated_at: now,
    };
    db.upsert_dream_entry(&row)?;
    Ok(row_to_dream(row))
}

pub fn list_dreams_for_date(db: &Database, sleep_date: &str) -> Result<Vec<DreamEntry>, CalendarError> {
    Ok(db
        .list_dreams_for_date(sleep_date)?
        .into_iter()
        .map(row_to_dream)
        .collect())
}

pub fn search_dreams(db: &Database, query: &str) -> Result<Vec<DreamEntry>, CalendarError> {
    Ok(db
        .search_dream_entries(query)?
        .into_iter()
        .map(row_to_dream)
        .collect())
}

pub fn update_dream(
    db: &Database,
    id: &str,
    input: UpdateDreamInput,
) -> Result<DreamEntry, CalendarError> {
    let mut row = db.get_dream_entry(id)?;
    if let Some(body) = input.body {
        let trimmed = body.trim();
        if trimmed.is_empty() {
            return Err(CalendarError::InvalidInput("dream body required".into()));
        }
        row.body = trimmed.to_string();
    }
    if let Some(title) = input.title {
        row.title = if title.trim().is_empty() {
            None
        } else {
            Some(title)
        };
    }
    if let Some(tags) = input.tags {
        row.tags_json = tags_to_json(&tags);
    }
    if input.mood.is_some() {
        row.mood = input.mood;
    }
    if input.sleep_quality.is_some() {
        row.sleep_quality = input.sleep_quality;
    }
    row.updated_at = chrono_now();
    db.upsert_dream_entry(&row)?;
    Ok(row_to_dream(row))
}

pub fn delete_dream(db: &Database, id: &str) -> Result<(), CalendarError> {
    db.delete_dream_entry(id)?;
    Ok(())
}

#[allow(dead_code)]
pub fn get_dream(db: &Database, id: &str) -> Result<DreamEntry, CalendarError> {
    Ok(row_to_dream(db.get_dream_entry(id)?))
}
