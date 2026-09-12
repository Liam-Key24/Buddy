use std::sync::Arc;

use buddy_database::{chrono_now, BuddyCalendarEventRow, Database};
use chrono::{Datelike, Duration, TimeZone, Utc};
use uuid::Uuid;

use crate::error::CalendarError;
use crate::models::{
    default_color_for_category, CreateEventInput, DateRange, Event, EventFilters, EventPriority,
    Flexibility, LifestyleScheduleRule, ReminderDelivery, ScheduleKind, ScheduleSegment,
    UpdateEventInput,
};
use crate::notifications::{
    dismiss_reminder, list_due_deliveries, list_notifications, mark_reminder_sent,
    parse_reminders, rebuild_reminders_for_event, serialize_reminders, snooze_reminder,
};
use crate::scheduling::{
    block_focus_time, compose_day_summary, compose_look_snapshot, constraint_after_work,
    constraint_weekend, detect_conflicts, find_free_slots, parse_duration_minutes, plan_day,
    punch_work_for_off_days, reschedule_flexible, resolve_when_range, resolve_window,
    schedule_items, ConflictReport, DayCapacity, DaySummary, FreeSlot, LookSnapshot,
    OrganizeItemIn, OrganizeMode, OrganizeResult, PlanDayRequest, PlanDayResult, ProposedBlock,
    ScheduleItem, ScheduleItemsResult, SchedulingContext, SchedulingPolicy, WriteEventOutcome,
};
use crate::services::recurrence::{
    expand_event_in_range, parse_recurrence, serialize_recurrence,
};

/// Read settings from SQLite (or any settings store) by key.
pub trait SettingsLookup: Send + Sync {
    fn get(&self, key: &str) -> Option<String>;
}

pub struct CalendarService {
    db: Arc<Database>,
    settings: Arc<dyn SettingsLookup>,
}

impl CalendarService {
    pub fn new(db: Arc<Database>, settings: Arc<dyn SettingsLookup>) -> Self {
        Self { db, settings }
    }

    pub fn with_db(db: Arc<Database>) -> Self {
        struct Empty;
        impl SettingsLookup for Empty {
            fn get(&self, _: &str) -> Option<String> {
                None
            }
        }
        Self {
            db,
            settings: Arc::new(Empty),
        }
    }

    fn default_timezone(&self) -> String {
        self.settings
            .get("calendar_default_timezone")
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| {
                // Prefer local offset approximation via Utc label; UI can override.
                "UTC".into()
            })
    }

    fn row_to_event(row: BuddyCalendarEventRow) -> Event {
        Event {
            id: row.id,
            title: row.title,
            description: row.description,
            location: row.location,
            category: row.category,
            color: row.color,
            start_time: row.start_time,
            end_time: row.end_time,
            all_day: row.all_day,
            timezone: row.timezone,
            recurrence: parse_recurrence(&row.recurrence_json),
            reminders: parse_reminders(&row.reminders_json),
            external_provider: row.external_provider,
            external_event_id: row.external_event_id,
            sync_status: row.sync_status,
            created_at: row.created_at,
            updated_at: row.updated_at,
            occurrence_of: None,
            flexibility: Flexibility::parse(&row.flexibility),
            priority: EventPriority::parse(&row.priority),
        }
    }

    fn event_to_row(event: &Event) -> BuddyCalendarEventRow {
        BuddyCalendarEventRow {
            id: event.id.clone(),
            title: event.title.clone(),
            description: event.description.clone(),
            location: event.location.clone(),
            category: event.category.clone(),
            color: event.color.clone(),
            start_time: event.start_time,
            end_time: event.end_time,
            all_day: event.all_day,
            timezone: event.timezone.clone(),
            recurrence_json: serialize_recurrence(&event.recurrence),
            reminders_json: serialize_reminders(&event.reminders),
            external_provider: event.external_provider.clone(),
            external_event_id: event.external_event_id.clone(),
            sync_status: event.sync_status.clone(),
            created_at: event.created_at,
            updated_at: event.updated_at,
            flexibility: event.flexibility.as_str().to_string(),
            priority: event.priority.as_str().to_string(),
        }
    }

    fn scheduling_policy(&self) -> SchedulingPolicy {
        let mut policy = SchedulingPolicy::default();
        if let Some(v) = self.settings.get("calendar_buffer_minutes") {
            if let Ok(n) = v.trim().parse::<u32>() {
                policy.buffer_minutes = n.clamp(0, 60);
            }
        }
        policy
    }

    async fn build_scheduling_context(
        &self,
        range: DateRange,
        allow_reduce_buffer: bool,
    ) -> Result<SchedulingContext, CalendarError> {
        let mut policy = self.scheduling_policy();
        policy.allow_reduce_buffer = allow_reduce_buffer;
        let events = self.list_events(range, EventFilters::default()).await?;
        let lifestyle_blocks =
            crate::services::schedule_service::list_blocks_in_range(&self.db, range.start, range.end)?;
        let lifestyle_blocks = punch_work_for_off_days(&events, lifestyle_blocks);
        Ok(SchedulingContext::new(
            events,
            lifestyle_blocks,
            policy,
            range,
        ))
    }

    fn validate_times(start: i64, end: i64) -> Result<(), CalendarError> {
        if end <= start {
            return Err(CalendarError::InvalidInput(
                "end_time must be after start_time".into(),
            ));
        }
        Ok(())
    }

    pub async fn list_events(
        &self,
        range: DateRange,
        filters: EventFilters,
    ) -> Result<Vec<Event>, CalendarError> {
        let rows = self.db.list_buddy_calendar_events(range.start, range.end)?;
        let mut events = Vec::new();
        for row in rows {
            let master = Self::row_to_event(row);
            let mut expanded = expand_event_in_range(&master, range.start, range.end);
            if !filters.categories.is_empty() {
                expanded.retain(|e| filters.categories.iter().any(|c| c == &e.category));
            }
            if let Some(q) = &filters.query {
                let q = q.trim().to_ascii_lowercase();
                if !q.is_empty() {
                    expanded.retain(|e| {
                        e.title.to_ascii_lowercase().contains(&q)
                            || e.description
                                .as_ref()
                                .map(|d| d.to_ascii_lowercase().contains(&q))
                                .unwrap_or(false)
                            || e.location
                                .as_ref()
                                .map(|d| d.to_ascii_lowercase().contains(&q))
                                .unwrap_or(false)
                    });
                }
            }
            events.extend(expanded);
        }
        events.sort_by_key(|e| e.start_time);
        Ok(events)
    }

    pub async fn get_event(&self, id: &str) -> Result<Event, CalendarError> {
        // Occurrence ids are master::timestamp
        let master_id = id.split("::").next().unwrap_or(id);
        let row = self.db.get_buddy_calendar_event(master_id)?;
        Ok(Self::row_to_event(row))
    }

    pub async fn create_event(&self, input: CreateEventInput) -> Result<Event, CalendarError> {
        match self.create_event_checked(input).await? {
            WriteEventOutcome::Ok { event } => Ok(event),
            WriteEventOutcome::Conflict { report } => Err(CalendarError::Conflict(
                serde_json::to_string_pretty(&report).unwrap_or_else(|_| "conflict".into()),
            )),
        }
    }

    /// Conflict-aware create. Returns suggestions instead of writing when blocked.
    pub async fn create_event_checked(
        &self,
        input: CreateEventInput,
    ) -> Result<WriteEventOutcome, CalendarError> {
        let title = input.title.trim().to_string();
        if title.is_empty() {
            return Err(CalendarError::InvalidInput("title is required".into()));
        }
        Self::validate_times(input.start_time, input.end_time)?;

        if !input.force && !input.all_day {
            let pad = self.scheduling_policy().buffer_ms() + (input.end_time - input.start_time);
            let range = DateRange {
                start: input.start_time - pad,
                end: input.end_time + pad,
            };
            let ctx = self.build_scheduling_context(range, false).await?;
            let report = detect_conflicts(&ctx, input.start_time, input.end_time, None);
            if report.has_conflicts {
                return Ok(WriteEventOutcome::Conflict { report });
            }
        }

        let category = input
            .category
            .filter(|c| !c.trim().is_empty())
            .unwrap_or_else(|| "general".into());
        let color = input
            .color
            .or_else(|| Some(default_color_for_category(&category).to_string()));
        let timezone = input
            .timezone
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| self.default_timezone());
        let now = chrono_now();
        let event = Event {
            id: Uuid::new_v4().to_string(),
            title,
            description: input.description,
            location: input.location,
            category,
            color,
            start_time: input.start_time,
            end_time: input.end_time,
            all_day: input.all_day,
            timezone,
            recurrence: input.recurrence,
            reminders: input.reminders,
            external_provider: None,
            external_event_id: None,
            sync_status: "local".into(),
            created_at: now,
            updated_at: now,
            occurrence_of: None,
            flexibility: input.flexibility.unwrap_or(Flexibility::Fixed),
            priority: input.priority.unwrap_or(EventPriority::Normal),
        };

        self.db
            .upsert_buddy_calendar_event(&Self::event_to_row(&event))?;
        rebuild_reminders_for_event(&self.db, &event)?;
        Ok(WriteEventOutcome::Ok { event })
    }

    pub async fn update_event(
        &self,
        id: &str,
        input: UpdateEventInput,
    ) -> Result<Event, CalendarError> {
        match self.update_event_checked(id, input).await? {
            WriteEventOutcome::Ok { event } => Ok(event),
            WriteEventOutcome::Conflict { report } => Err(CalendarError::Conflict(
                serde_json::to_string_pretty(&report).unwrap_or_else(|_| "conflict".into()),
            )),
        }
    }

    pub async fn update_event_checked(
        &self,
        id: &str,
        input: UpdateEventInput,
    ) -> Result<WriteEventOutcome, CalendarError> {
        let master_id = id.split("::").next().unwrap_or(id);
        let mut event = Self::row_to_event(self.db.get_buddy_calendar_event(master_id)?);

        if let Some(title) = input.title {
            let t = title.trim().to_string();
            if t.is_empty() {
                return Err(CalendarError::InvalidInput("title is required".into()));
            }
            event.title = t;
        }
        if let Some(desc) = input.description {
            event.description = if desc.trim().is_empty() {
                None
            } else {
                Some(desc)
            };
        }
        if let Some(loc) = input.location {
            event.location = if loc.trim().is_empty() {
                None
            } else {
                Some(loc)
            };
        }
        if let Some(cat) = input.category {
            event.category = cat;
            if event.color.is_none() {
                event.color = Some(default_color_for_category(&event.category).to_string());
            }
        }
        if let Some(color) = input.color {
            event.color = if color.trim().is_empty() {
                None
            } else {
                Some(color)
            };
        }
        if let Some(start) = input.start_time {
            event.start_time = start;
        }
        if let Some(end) = input.end_time {
            event.end_time = end;
        }
        if let Some(all_day) = input.all_day {
            event.all_day = all_day;
        }
        if let Some(tz) = input.timezone {
            event.timezone = tz;
        }
        if input.clear_recurrence {
            event.recurrence = None;
        } else if let Some(rec) = input.recurrence {
            event.recurrence = Some(rec);
        }
        if let Some(reminders) = input.reminders {
            event.reminders = reminders;
        }
        if let Some(flex) = input.flexibility {
            event.flexibility = flex;
        }
        if let Some(priority) = input.priority {
            event.priority = priority;
        }

        Self::validate_times(event.start_time, event.end_time)?;

        if !input.force && !event.all_day {
            let pad = self.scheduling_policy().buffer_ms() + (event.end_time - event.start_time);
            let range = DateRange {
                start: event.start_time - pad,
                end: event.end_time + pad,
            };
            let ctx = self.build_scheduling_context(range, false).await?;
            let report =
                detect_conflicts(&ctx, event.start_time, event.end_time, Some(&event.id));
            if report.has_conflicts {
                return Ok(WriteEventOutcome::Conflict { report });
            }
        }

        event.updated_at = chrono_now();

        self.db
            .upsert_buddy_calendar_event(&Self::event_to_row(&event))?;
        rebuild_reminders_for_event(&self.db, &event)?;
        Ok(WriteEventOutcome::Ok { event })
    }

    pub async fn delete_event(&self, id: &str) -> Result<(), CalendarError> {
        let master_id = id.split("::").next().unwrap_or(id);
        self.db.delete_buddy_calendar_event(master_id)?;
        Ok(())
    }

    pub async fn delete_all_events(&self) -> Result<usize, CalendarError> {
        Ok(self.db.delete_all_buddy_calendar_events()?)
    }

    pub async fn delete_events_matching(&self, query: &str) -> Result<Vec<String>, CalendarError> {
        let matches = self.db.search_buddy_calendar_events(query, None, None)?;
        let mut deleted = Vec::new();
        for row in matches {
            self.db.delete_buddy_calendar_event(&row.id)?;
            deleted.push(row.id);
        }
        Ok(deleted)
    }

    pub async fn duplicate_event(&self, id: &str) -> Result<Event, CalendarError> {
        let master_id = id.split("::").next().unwrap_or(id);
        let src = Self::row_to_event(self.db.get_buddy_calendar_event(master_id)?);
        let duration = src.end_time - src.start_time;
        let now = chrono_now();
        let event = Event {
            id: Uuid::new_v4().to_string(),
            title: format!("{} (copy)", src.title),
            description: src.description,
            location: src.location,
            category: src.category,
            color: src.color,
            start_time: src.start_time + Duration::days(1).num_milliseconds(),
            end_time: src.start_time + Duration::days(1).num_milliseconds() + duration,
            all_day: src.all_day,
            timezone: src.timezone,
            recurrence: src.recurrence,
            reminders: src.reminders,
            external_provider: None,
            external_event_id: None,
            sync_status: "local".into(),
            created_at: now,
            updated_at: now,
            occurrence_of: None,
            flexibility: src.flexibility,
            priority: src.priority,
        };
        self.db.upsert_buddy_calendar_event(&Self::event_to_row(&event))?;
        rebuild_reminders_for_event(&self.db, &event)?;
        Ok(event)
    }

    pub async fn search_events(
        &self,
        query: &str,
        range: Option<DateRange>,
    ) -> Result<Vec<Event>, CalendarError> {
        let (start, end) = match range {
            Some(r) => (Some(r.start), Some(r.end)),
            None => (None, None),
        };
        let rows = self.db.search_buddy_calendar_events(query, start, end)?;
        Ok(rows.into_iter().map(Self::row_to_event).collect())
    }

    pub async fn get_today(&self) -> Result<Vec<Event>, CalendarError> {
        let (start, end) = local_day_bounds(0);
        self.list_events(
            DateRange { start, end },
            EventFilters::default(),
        )
        .await
    }

    pub async fn get_tomorrow(&self) -> Result<Vec<Event>, CalendarError> {
        let (start, end) = local_day_bounds(1);
        self.list_events(
            DateRange { start, end },
            EventFilters::default(),
        )
        .await
    }

    pub async fn get_this_week(&self) -> Result<Vec<Event>, CalendarError> {
        let range = resolve_window(Some("this_week"));
        self.list_events(range, EventFilters::default()).await
    }

    // --- Reminders / notifications ---

    pub async fn list_due_reminders(&self, now_ms: i64) -> Result<Vec<ReminderDelivery>, CalendarError> {
        list_due_deliveries(&self.db, now_ms)
    }

    pub async fn mark_reminder_sent(&self, id: &str) -> Result<(), CalendarError> {
        mark_reminder_sent(&self.db, id)
    }

    pub async fn snooze_reminder(&self, id: &str, minutes: u32) -> Result<(), CalendarError> {
        snooze_reminder(&self.db, id, minutes)
    }

    pub async fn dismiss_reminder(&self, id: &str) -> Result<(), CalendarError> {
        dismiss_reminder(&self.db, id)
    }

    pub async fn list_notifications(&self) -> Result<Vec<ReminderDelivery>, CalendarError> {
        list_notifications(&self.db)
    }

    pub async fn notification_count(&self) -> Result<i64, CalendarError> {
        Ok(self.db.count_pending_reminders()?)
    }

    pub fn notifications_enabled(&self) -> bool {
        self.settings
            .get("calendar_notifications_enabled")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(true)
    }

    // --- Lifestyle (work / sleep / dreams) ---

    pub async fn list_schedule_blocks(
        &self,
        start: i64,
        end: i64,
    ) -> Result<Vec<crate::models::ScheduleBlock>, CalendarError> {
        let events = self
            .list_events(DateRange { start, end }, EventFilters::default())
            .await?;
        let blocks =
            crate::services::schedule_service::list_blocks_in_range(&self.db, start, end)?;
        Ok(punch_work_for_off_days(&events, blocks))
    }

    pub async fn list_schedule_rules(&self) -> Result<Vec<LifestyleScheduleRule>, CalendarError> {
        let rows = self.db.list_lifestyle_schedule_rules()?;
        rows.into_iter().map(row_to_schedule_rule).collect()
    }

    pub async fn set_schedule_rule(
        &self,
        kind: ScheduleKind,
        segments: Vec<ScheduleSegment>,
    ) -> Result<LifestyleScheduleRule, CalendarError> {
        if segments.is_empty() {
            return Err(CalendarError::InvalidInput(
                "schedule needs at least one segment".into(),
            ));
        }
        let normalized = normalize_segments(segments)?;
        let updated_at = chrono_now();
        let json = serde_json::to_string(&normalized)
            .map_err(|e| CalendarError::InvalidInput(e.to_string()))?;
        self.db
            .upsert_lifestyle_schedule_rule(kind.as_str(), &json, updated_at)?;
        Ok(LifestyleScheduleRule {
            kind,
            segments: normalized,
            updated_at,
        })
    }

    /// Update clock times on existing segments (keep weekdays), or seed a default week.
    pub async fn set_schedule_times(
        &self,
        kind: ScheduleKind,
        start_hm: &str,
        end_hm: &str,
    ) -> Result<LifestyleScheduleRule, CalendarError> {
        let (sh, sm) = crate::services::schedule_service::parse_hm(start_hm)?;
        let (eh, em) = crate::services::schedule_service::parse_hm(end_hm)?;
        let start_hm = format!("{sh:02}:{sm:02}");
        let end_hm = format!("{eh:02}:{em:02}");
        let crosses = hm_minutes(&start_hm)? >= hm_minutes(&end_hm)?;
        let existing = self.db.get_lifestyle_schedule_rule(kind.as_str()).ok();
        let mut segments: Vec<ScheduleSegment> = match existing {
            Some(row) => serde_json::from_str(&row.segments_json)
                .map_err(|e| CalendarError::InvalidInput(e.to_string()))?,
            None => Vec::new(),
        };
        if segments.is_empty() {
            let by_day = match kind {
                ScheduleKind::Work => vec!["MO", "TU", "WE", "TH", "FR"],
                ScheduleKind::Sleep => vec!["MO", "TU", "WE", "TH", "FR", "SA", "SU"],
            }
            .into_iter()
            .map(str::to_string)
            .collect();
            segments.push(ScheduleSegment {
                by_day,
                start_hm: start_hm.clone(),
                end_hm: end_hm.clone(),
                crosses_midnight: crosses,
            });
        } else {
            for seg in &mut segments {
                seg.start_hm = start_hm.clone();
                seg.end_hm = end_hm.clone();
                seg.crosses_midnight = crosses;
            }
        }
        self.set_schedule_rule(kind, segments).await
    }

    pub async fn log_dream(
        &self,
        input: crate::models::CreateDreamInput,
    ) -> Result<crate::models::DreamEntry, CalendarError> {
        crate::services::dream_service::log_dream(&self.db, input)
    }

    pub async fn list_dreams(
        &self,
        sleep_date: &str,
    ) -> Result<Vec<crate::models::DreamEntry>, CalendarError> {
        crate::services::dream_service::list_dreams_for_date(&self.db, sleep_date)
    }

    pub async fn search_dreams(
        &self,
        query: &str,
    ) -> Result<Vec<crate::models::DreamEntry>, CalendarError> {
        crate::services::dream_service::search_dreams(&self.db, query)
    }

    pub async fn update_dream(
        &self,
        id: &str,
        input: crate::models::UpdateDreamInput,
    ) -> Result<crate::models::DreamEntry, CalendarError> {
        crate::services::dream_service::update_dream(&self.db, id, input)
    }

    pub async fn delete_dream(&self, id: &str) -> Result<(), CalendarError> {
        crate::services::dream_service::delete_dream(&self.db, id)
    }

    pub async fn get_work_stats(&self) -> Result<crate::models::WorkStats, CalendarError> {
        crate::services::work_service::get_stats(&self.db)
    }

    pub async fn log_work_sales(
        &self,
        work_date: Option<String>,
        amount: f64,
        currency: Option<String>,
    ) -> Result<crate::models::WorkDayLog, CalendarError> {
        crate::services::work_service::log_sales(&self.db, work_date, amount, currency)
    }

    pub async fn set_work_hours(
        &self,
        work_date: Option<String>,
        actual_start_ms: Option<i64>,
        actual_end_ms: Option<i64>,
    ) -> Result<crate::models::WorkDayLog, CalendarError> {
        crate::services::work_service::set_hours(
            &self.db,
            work_date,
            actual_start_ms,
            actual_end_ms,
        )
    }

    pub async fn get_work_day_log(
        &self,
        work_date: &str,
    ) -> Result<crate::models::WorkDayLog, CalendarError> {
        crate::services::work_service::get_or_empty(&self.db, work_date)
    }

    pub async fn last_sleep_date(&self) -> Result<String, CalendarError> {
        crate::services::schedule_service::last_sleep_date(&self.db, None)
    }

    // --- AI scheduling ---

    pub async fn find_free_time(
        &self,
        duration_minutes: u32,
        range: DateRange,
        limit: Option<usize>,
        allow_reduce_buffer: bool,
    ) -> Result<Vec<FreeSlot>, CalendarError> {
        let ctx = self
            .build_scheduling_context(range, allow_reduce_buffer)
            .await?;
        let duration_ms = duration_minutes.max(1) as i64 * 60_000;
        Ok(find_free_slots(
            &ctx,
            duration_ms,
            limit.unwrap_or(5),
            None,
        ))
    }

    pub async fn detect_event_conflicts(
        &self,
        start: i64,
        end: i64,
        exclude_event_id: Option<String>,
        allow_reduce_buffer: bool,
    ) -> Result<ConflictReport, CalendarError> {
        let pad = self.scheduling_policy().buffer_ms() + (end - start);
        let range = DateRange {
            start: start - pad,
            end: end + pad,
        };
        let ctx = self
            .build_scheduling_context(range, allow_reduce_buffer)
            .await?;
        Ok(detect_conflicts(
            &ctx,
            start,
            end,
            exclude_event_id.as_deref(),
        ))
    }

    pub async fn get_capacity(
        &self,
        day_ms: i64,
    ) -> Result<DayCapacity, CalendarError> {
        let (start, end) = crate::scheduling::local_day_bounds_ms(day_ms);
        let ctx = self
            .build_scheduling_context(DateRange { start, end }, false)
            .await?;
        Ok(crate::scheduling::compute_day_capacity(&ctx, day_ms))
    }

    pub async fn day_summary(&self, day_ms: i64) -> Result<DaySummary, CalendarError> {
        let (start, end) = crate::scheduling::local_day_bounds_ms(day_ms);
        let ctx = self
            .build_scheduling_context(DateRange { start, end }, false)
            .await?;
        Ok(compose_day_summary(&ctx, day_ms))
    }

    pub async fn look(
        &self,
        when: Option<String>,
        focus: Option<String>,
        query: Option<String>,
        duration_minutes: Option<u32>,
    ) -> Result<LookSnapshot, CalendarError> {
        let range = resolve_when_range(when.as_deref());
        let focus = focus.unwrap_or_else(|| "events".into()).to_ascii_lowercase();
        let ctx = self.build_scheduling_context(range, false).await?;
        let slots = if focus == "free" || focus == "slots" {
            let duration = duration_minutes.unwrap_or(60).max(15) as i64 * 60_000;
            find_free_slots(&ctx, duration, 8, None)
        } else {
            vec![]
        };
        let mut snap = compose_look_snapshot(
            &ctx,
            when,
            &focus,
            slots,
            query.as_deref(),
        );
        if focus == "work" {
            snap.events.clear();
            snap.schedule
                .retain(|b| b.kind == crate::models::ScheduleKind::Work);
            if snap.schedule.is_empty() {
                let next_range = DateRange {
                    start: range.end,
                    end: range.end + 8 * 86_400_000,
                };
                let next = self.build_scheduling_context(next_range, false).await?;
                if let Some(block) = next
                    .lifestyle_blocks
                    .iter()
                    .find(|b| b.kind == crate::models::ScheduleKind::Work)
                {
                    snap.schedule
                        .push(crate::scheduling::look_block(block));
                }
            }
        }
        Ok(snap)
    }

    pub async fn organize(
        &self,
        window: Option<String>,
        items: Vec<OrganizeItemIn>,
        constraints: Vec<String>,
        mode: OrganizeMode,
        previous_items: Option<Vec<OrganizeItemIn>>,
        lift_event_ids: Option<Vec<String>>,
    ) -> Result<OrganizeResult, CalendarError> {
        let mut range = resolve_window(window.as_deref());
        let mut working_items = items;
        if working_items.is_empty() {
            if let Some(prev) = previous_items {
                working_items = prev;
            }
        }

        let mut lifted_ids = lift_event_ids.unwrap_or_default();
        let mut ctx = self.build_scheduling_context(range, false).await?;
        if !lifted_ids.is_empty() {
            ctx.events
                .retain(|e| !lifted_ids.iter().any(|id| id == &e.id || e.id.starts_with(&format!("{id}::"))));
        }

        // Rebalance: no new titles — lift flexible events in window and reschedule them.
        if working_items.is_empty() {
            let movable: Vec<Event> = ctx
                .events
                .iter()
                .filter(|e| {
                    e.flexibility.is_movable()
                        && e.start_time < range.end
                        && e.end_time > range.start
                })
                .cloned()
                .collect();
            for ev in &movable {
                lifted_ids.push(ev.id.clone());
                let mins = ((ev.end_time - ev.start_time).max(60_000) / 60_000) as u32;
                working_items.push(OrganizeItemIn {
                    title: ev.title.clone(),
                    duration: None,
                    duration_minutes: Some(mins.max(15)),
                    when: None,
                    count: Some(1),
                });
            }
            ctx.events.retain(|e| !lifted_ids.iter().any(|id| id == &e.id));
        }

        let default_mins = self
            .settings
            .get("preferred_activity_duration")
            .and_then(|s| s.parse::<u32>().ok())
            .filter(|&n| n >= 15)
            .unwrap_or(60);

        let after_work_all = constraint_after_work(&constraints, None);
        let weekend_all = constraint_weekend(&constraints, None);
        if weekend_all {
            range = resolve_window(Some("weekend"));
            ctx = self.build_scheduling_context(range, false).await?;
            ctx.events.retain(|e| !lifted_ids.iter().any(|id| id == &e.id));
        }

        let schedule_items_in: Vec<ScheduleItem> = working_items
            .iter()
            .map(|item| {
                let mins = item
                    .duration_minutes
                    .or_else(|| item.duration.as_deref().and_then(parse_duration_minutes))
                    .unwrap_or(default_mins);
                let after = after_work_all
                    || constraint_after_work(&constraints, item.when.as_deref());
                let weekend = weekend_all || constraint_weekend(&constraints, item.when.as_deref());
                ScheduleItem {
                    title: item.title.clone(),
                    duration_minutes: mins,
                    deadline: None,
                    priority: None,
                    flexibility: Some(Flexibility::Flexible),
                    category: Some("personal".into()),
                    description: weekend.then(|| "prefer weekend".into()),
                    count: item.count,
                    prefer_spread: Some(item.count.unwrap_or(1) > 1),
                    prefer_after_work: Some(after),
                    spark_id: None,
                }
            })
            .collect();

        if schedule_items_in.is_empty() {
            return Ok(OrganizeResult {
                status: "empty".into(),
                window_start: range.start,
                window_end: range.end,
                scheduled: vec![],
                unscheduled: vec![],
                suggestions: vec![],
                lifted_ids,
                apply: false,
                tradeoff: None,
            });
        }

        let apply = matches!(mode, OrganizeMode::Commit);
        if apply && !lifted_ids.is_empty() {
            for id in &lifted_ids {
                let _ = self.delete_event(id).await;
            }
        }

        let mut result = schedule_items(&ctx, &schedule_items_in);
        if apply {
            result.scheduled = self.persist_proposals(&result.scheduled).await?;
        }

        Ok(OrganizeResult {
            status: if result.tradeoff.is_some() {
                "needs_tradeoff".into()
            } else if apply {
                "committed".into()
            } else {
                "proposed".into()
            },
            window_start: range.start,
            window_end: range.end,
            scheduled: result.scheduled,
            unscheduled: result.unscheduled,
            suggestions: result.suggestions,
            lifted_ids,
            apply,
            tradeoff: result.tradeoff,
        })
    }

    pub async fn schedule_task_items(
        &self,
        items: Vec<ScheduleItem>,
        range: DateRange,
        apply: bool,
    ) -> Result<ScheduleItemsResult, CalendarError> {
        let ctx = self.build_scheduling_context(range, false).await?;
        let mut result = schedule_items(&ctx, &items);
        if apply {
            result.scheduled = self.persist_proposals(&result.scheduled).await?;
        }
        Ok(result)
    }

    pub async fn plan_my_day(
        &self,
        request: PlanDayRequest,
    ) -> Result<PlanDayResult, CalendarError> {
        let (start, end) = crate::scheduling::local_day_bounds_ms(request.day);
        let ctx = self
            .build_scheduling_context(DateRange { start, end }, false)
            .await?;
        let mut result = plan_day(&ctx, &request);
        if request.apply {
            result.proposed = self.persist_proposals(&result.proposed).await?;
        }
        Ok(result)
    }

    pub async fn block_time(
        &self,
        title: String,
        duration_minutes: u32,
        range: DateRange,
        apply: bool,
    ) -> Result<Option<ProposedBlock>, CalendarError> {
        let ctx = self.build_scheduling_context(range, false).await?;
        let Some(mut proposed) = block_focus_time(&ctx, &title, duration_minutes) else {
            return Ok(None);
        };
        if apply {
            let persisted = self.persist_proposals(std::slice::from_ref(&proposed)).await?;
            proposed = persisted.into_iter().next().unwrap_or(proposed);
        }
        Ok(Some(proposed))
    }

    pub async fn resolve_conflict(
        &self,
        title: String,
        start: i64,
        end: i64,
        flexibility: Option<Flexibility>,
        priority: Option<EventPriority>,
        category: Option<String>,
        description: Option<String>,
    ) -> Result<Event, CalendarError> {
        let mut input = CreateEventInput {
            title,
            description,
            location: None,
            category,
            color: None,
            start_time: start,
            end_time: end,
            all_day: false,
            timezone: None,
            recurrence: None,
            reminders: vec![],
            flexibility,
            priority,
            force: true,
        };
        // force writes after user chose a resolution slot
        input.force = true;
        self.create_event(input).await
    }

    pub async fn suggest_reschedule(
        &self,
        event_id: &str,
        range: DateRange,
    ) -> Result<Vec<FreeSlot>, CalendarError> {
        let event = self.get_event(event_id).await?;
        let ctx = self.build_scheduling_context(range, false).await?;
        reschedule_flexible(&ctx, &event).map_err(CalendarError::InvalidInput)
    }

    async fn persist_proposals(
        &self,
        proposed: &[ProposedBlock],
    ) -> Result<Vec<ProposedBlock>, CalendarError> {
        let mut out = Vec::new();
        for block in proposed {
            let created = self
                .create_event(CreateEventInput {
                    title: block.title.clone(),
                    description: block.description.clone(),
                    location: None,
                    category: block.category.clone(),
                    color: None,
                    start_time: block.start,
                    end_time: block.end,
                    all_day: false,
                    timezone: None,
                    recurrence: None,
                    reminders: vec![],
                    flexibility: Some(block.flexibility),
                    priority: Some(block.priority),
                    force: true,
                })
                .await?;
            out.push(ProposedBlock {
                title: created.title,
                start: created.start_time,
                end: created.end_time,
                flexibility: created.flexibility,
                priority: created.priority,
                category: Some(created.category),
                description: created.description,
                score: block.score,
                reasons: block.reasons.clone(),
            });
        }
        Ok(out)
    }
}

fn row_to_schedule_rule(
    row: buddy_database::LifestyleScheduleRuleRow,
) -> Result<LifestyleScheduleRule, CalendarError> {
    let kind = ScheduleKind::parse(&row.kind)
        .ok_or_else(|| CalendarError::InvalidInput(format!("bad schedule kind {}", row.kind)))?;
    let segments: Vec<ScheduleSegment> = serde_json::from_str(&row.segments_json)
        .map_err(|e| CalendarError::InvalidInput(e.to_string()))?;
    Ok(LifestyleScheduleRule {
        kind,
        segments,
        updated_at: row.updated_at,
    })
}

fn hm_minutes(hm: &str) -> Result<u32, CalendarError> {
    let (h, m) = crate::services::schedule_service::parse_hm(hm)?;
    Ok(h * 60 + m)
}

fn normalize_segments(segments: Vec<ScheduleSegment>) -> Result<Vec<ScheduleSegment>, CalendarError> {
    let mut out = Vec::new();
    for mut seg in segments {
        if seg.by_day.is_empty() {
            return Err(CalendarError::InvalidInput(
                "segment by_day cannot be empty".into(),
            ));
        }
        let (sh, sm) = crate::services::schedule_service::parse_hm(&seg.start_hm)?;
        let (eh, em) = crate::services::schedule_service::parse_hm(&seg.end_hm)?;
        seg.start_hm = format!("{sh:02}:{sm:02}");
        seg.end_hm = format!("{eh:02}:{em:02}");
        if hm_minutes(&seg.start_hm)? >= hm_minutes(&seg.end_hm)? {
            seg.crosses_midnight = true;
        }
        out.push(seg);
    }
    Ok(out)
}

/// Day bounds in local-ish UTC (midnight UTC + day_offset). Sufficient for AI tools.
fn local_day_bounds(day_offset: i64) -> (i64, i64) {
    let today = Utc::now().date_naive() + Duration::days(day_offset);
    let start = Utc
        .from_utc_datetime(&today.and_hms_opt(0, 0, 0).unwrap())
        .timestamp_millis();
    let end = start + Duration::days(1).num_milliseconds();
    (start, end)
}

/// Expand a month cursor into a buffered range covering the full month grid.
pub fn month_buffer_range(year: i32, month: u32) -> DateRange {
    let start_naive = chrono::NaiveDate::from_ymd_opt(year, month, 1)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap();
    let start = Utc.from_utc_datetime(&start_naive).timestamp_millis();
    let next_month = if month == 12 {
        chrono::NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        chrono::NaiveDate::from_ymd_opt(year, month + 1, 1)
    }
    .unwrap()
    .and_hms_opt(0, 0, 0)
    .unwrap();
    let end = Utc.from_utc_datetime(&next_month).timestamp_millis();
    // Buffer ±7 days for month grid spillover
    DateRange {
        start: start - Duration::days(7).num_milliseconds(),
        end: end + Duration::days(7).num_milliseconds(),
    }
}
