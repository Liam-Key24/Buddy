-- AI scheduling fields for BUDDY calendar events.
ALTER TABLE buddy_calendar_events ADD COLUMN flexibility TEXT NOT NULL DEFAULT 'fixed';
ALTER TABLE buddy_calendar_events ADD COLUMN priority TEXT NOT NULL DEFAULT 'normal';
