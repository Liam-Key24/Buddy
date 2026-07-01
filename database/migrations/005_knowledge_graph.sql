CREATE TABLE IF NOT EXISTS kg_entities (
    id TEXT PRIMARY KEY,
    workspace_path TEXT NOT NULL,
    name TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    source_kind TEXT,
    source_id TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS kg_relations (
    id TEXT PRIMARY KEY,
    workspace_path TEXT NOT NULL,
    from_entity_id TEXT NOT NULL,
    to_entity_id TEXT NOT NULL,
    relation_type TEXT NOT NULL,
    weight REAL NOT NULL DEFAULT 1.0,
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_kg_entities_workspace ON kg_entities(workspace_path);
CREATE INDEX IF NOT EXISTS idx_kg_entities_name ON kg_entities(workspace_path, name);
CREATE INDEX IF NOT EXISTS idx_kg_relations_workspace ON kg_relations(workspace_path);
CREATE INDEX IF NOT EXISTS idx_kg_relations_from ON kg_relations(from_entity_id);
CREATE INDEX IF NOT EXISTS idx_kg_relations_to ON kg_relations(to_entity_id);
