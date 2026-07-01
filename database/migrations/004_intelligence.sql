ALTER TABLE memory_working ADD COLUMN search_text TEXT;
ALTER TABLE memory_project ADD COLUMN search_text TEXT;
ALTER TABLE memory_preference ADD COLUMN search_text TEXT;
ALTER TABLE memory_handover ADD COLUMN search_text TEXT;
ALTER TABLE memory_decision ADD COLUMN search_text TEXT;
ALTER TABLE memory_error ADD COLUMN search_text TEXT;
ALTER TABLE memory_tool ADD COLUMN search_text TEXT;
ALTER TABLE memory_reflection ADD COLUMN search_text TEXT;

CREATE INDEX IF NOT EXISTS idx_memory_working_search ON memory_working(workspace_path, updated_at);
CREATE INDEX IF NOT EXISTS idx_memory_project_search ON memory_project(workspace_path, updated_at);
CREATE INDEX IF NOT EXISTS idx_memory_preference_search ON memory_preference(workspace_path, updated_at);
CREATE INDEX IF NOT EXISTS idx_memory_handover_search ON memory_handover(workspace_path, updated_at);
CREATE INDEX IF NOT EXISTS idx_memory_decision_search ON memory_decision(workspace_path, updated_at);
CREATE INDEX IF NOT EXISTS idx_memory_error_search ON memory_error(workspace_path, updated_at);
CREATE INDEX IF NOT EXISTS idx_memory_tool_search ON memory_tool(workspace_path, updated_at);
CREATE INDEX IF NOT EXISTS idx_memory_reflection_search ON memory_reflection(workspace_path, updated_at);
