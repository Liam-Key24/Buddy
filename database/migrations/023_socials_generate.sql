ALTER TABLE social_ideas ADD COLUMN used_at INTEGER;
ALTER TABLE social_drafts ADD COLUMN archived INTEGER NOT NULL DEFAULT 0;
ALTER TABLE social_drafts ADD COLUMN source_post_id TEXT;
