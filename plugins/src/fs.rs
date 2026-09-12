use std::sync::Arc;

use buddy_core::{excluded_paths_from_setting, parse_tool_json, PathGuard, Tool, ToolError, ToolResult};
use buddy_database::Database;
use serde::Deserialize;

const MAX_READ_BYTES: u64 = 512 * 1024;
const MAX_WRITE_BYTES: usize = 512 * 1024;
const MAX_LIST_ENTRIES: usize = 500;

fn reject_oversize(content: &str, op: &str) -> Result<(), ToolError> {
    if content.len() > MAX_WRITE_BYTES {
        return Err(ToolError::ExecutionFailed(format!(
            "{op} content too large ({} bytes, limit {MAX_WRITE_BYTES})",
            content.len()
        )));
    }
    Ok(())
}

fn build_guard(db: &Database) -> Result<PathGuard, ToolError> {
    let excluded = excluded_paths_from_setting(db.get_setting("fs_excluded_paths").ok().flatten());
    Ok(PathGuard::home(excluded)?)
}

fn parse<T: for<'de> Deserialize<'de>>(input: &str, tool: &str) -> Result<T, ToolError> {
    parse_tool_json(input, tool)
}

pub struct ReadFileTool {
    db: Arc<Database>,
}

pub struct WriteFileTool {
    db: Arc<Database>,
}

pub struct EditFileTool {
    db: Arc<Database>,
}

pub struct DeleteFileTool {
    db: Arc<Database>,
}

pub struct ListDirTool {
    db: Arc<Database>,
}

impl ReadFileTool {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }
}
impl WriteFileTool {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }
}
impl EditFileTool {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }
}
impl DeleteFileTool {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }
}
impl ListDirTool {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }
}

#[derive(Debug, Deserialize)]
struct PathInput {
    path: String,
}

#[derive(Debug, Deserialize)]
struct WriteInput {
    path: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct EditInput {
    path: String,
    #[serde(default)]
    old: Option<String>,
    #[serde(default)]
    new: Option<String>,
    #[serde(default)]
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ListInput {
    path: String,
    #[serde(default)]
    depth: Option<usize>,
}

impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: PathInput = parse(input, "read_file")?;
        let guard = build_guard(&self.db)?;
        let path = guard
            .check(&parsed.path)?;

        let metadata = std::fs::metadata(&path)
            .map_err(|e| ToolError::ExecutionFailed(format!("cannot stat {}: {e}", path.display())))?;
        if metadata.len() > MAX_READ_BYTES {
            return Err(ToolError::ExecutionFailed(format!(
                "file too large ({} bytes, limit {MAX_READ_BYTES})",
                metadata.len()
            )));
        }

        let content = std::fs::read_to_string(&path).map_err(|e| {
            ToolError::ExecutionFailed(format!("cannot read {}: {e}", path.display()))
        })?;
        Ok(ToolResult { output: content })
    }
}

impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: WriteInput = parse(input, "write_file")?;
        reject_oversize(&parsed.content, "write_file")?;
        let guard = build_guard(&self.db)?;
        let path = guard
            .check(&parsed.path)?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                ToolError::ExecutionFailed(format!("cannot create {}: {e}", parent.display()))
            })?;
        }
        std::fs::write(&path, &parsed.content).map_err(|e| {
            ToolError::ExecutionFailed(format!("cannot write {}: {e}", path.display()))
        })?;

        let _ = self
            .db
            .log_file_operation(&path.display().to_string(), "write", None);
        Ok(ToolResult {
            output: format!("Wrote {} bytes to {}", parsed.content.len(), path.display()),
        })
    }
}

impl Tool for EditFileTool {
    fn name(&self) -> &str {
        "edit_file"
    }

    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: EditInput = parse(input, "edit_file")?;
        let guard = build_guard(&self.db)?;
        let path = guard
            .check(&parsed.path)?;

        let (new_content, summary) = if let Some(full) = parsed.content {
            reject_oversize(&full, "edit_file")?;
            (full, "replaced file contents".to_string())
        } else {
            let old = parsed.old.ok_or_else(|| {
                ToolError::ExecutionFailed(
                    "edit_file requires either \"content\" or both \"old\" and \"new\"".into(),
                )
            })?;
            let new = parsed.new.unwrap_or_default();
            let existing = std::fs::read_to_string(&path).map_err(|e| {
                ToolError::ExecutionFailed(format!("cannot read {}: {e}", path.display()))
            })?;
            if !existing.contains(&old) {
                return Err(ToolError::ExecutionFailed(
                    "\"old\" text not found in file".into(),
                ));
            }
            let count = existing.matches(&old).count();
            (existing.replacen(&old, &new, 1), format!("{count} match(es) found, replaced first"))
        };
        reject_oversize(&new_content, "edit_file")?;

        std::fs::write(&path, &new_content).map_err(|e| {
            ToolError::ExecutionFailed(format!("cannot write {}: {e}", path.display()))
        })?;
        let _ = self
            .db
            .log_file_operation(&path.display().to_string(), "edit", None);
        Ok(ToolResult {
            output: format!("Edited {} ({summary})", path.display()),
        })
    }
}

impl Tool for DeleteFileTool {
    fn name(&self) -> &str {
        "delete_file"
    }

    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: PathInput = parse(input, "delete_file")?;
        let guard = build_guard(&self.db)?;
        let path = guard
            .check(&parsed.path)?;

        if !path.exists() {
            return Err(ToolError::ExecutionFailed(format!(
                "{} does not exist",
                path.display()
            )));
        }
        if path.is_dir() {
            return Err(ToolError::ExecutionFailed(
                "delete_file refuses to delete directories".into(),
            ));
        }
        std::fs::remove_file(&path).map_err(|e| {
            ToolError::ExecutionFailed(format!("cannot delete {}: {e}", path.display()))
        })?;
        let _ = self
            .db
            .log_file_operation(&path.display().to_string(), "delete", None);
        Ok(ToolResult {
            output: format!("Deleted {}", path.display()),
        })
    }
}

impl Tool for ListDirTool {
    fn name(&self) -> &str {
        "list_dir"
    }

    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: ListInput = parse(input, "list_dir")?;
        let guard = build_guard(&self.db)?;
        let root = guard
            .check(&parsed.path)?;
        let depth = parsed.depth.unwrap_or(1).min(4);

        let mut lines = Vec::new();
        list_recursive(&guard, &root, depth, &mut lines);
        if lines.len() >= MAX_LIST_ENTRIES {
            lines.push(format!("(truncated at {MAX_LIST_ENTRIES} entries)"));
        } else if lines.is_empty() {
            lines.push("(empty)".to_string());
        }
        Ok(ToolResult {
            output: lines.join("\n"),
        })
    }
}

fn list_recursive(guard: &PathGuard, dir: &std::path::Path, depth: usize, out: &mut Vec<String>) {
    if depth == 0 || out.len() >= MAX_LIST_ENTRIES {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut items: Vec<_> = entries.filter_map(|e| e.ok()).collect();
    items.sort_by_key(|e| e.file_name());
    for entry in items {
        if out.len() >= MAX_LIST_ENTRIES {
            return;
        }
        let path = entry.path();
        // Skip anything the guard would reject (e.g. excluded subtrees).
        if guard.check(&path.display().to_string()).is_err() {
            continue;
        }
        let is_dir = path.is_dir();
        let display = path.display().to_string();
        out.push(if is_dir {
            format!("{display}/")
        } else {
            display.clone()
        });
        if is_dir {
            list_recursive(guard, &path, depth - 1, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_rejects_oversize_content() {
        let big = "x".repeat(MAX_WRITE_BYTES + 1);
        assert!(reject_oversize(&big, "write_file").is_err());
        assert!(reject_oversize("ok", "write_file").is_ok());
    }

    #[test]
    fn list_recursive_respects_entry_cap() {
        let dir = std::env::temp_dir().join(format!(
            "buddy-fs-list-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        for i in 0..(MAX_LIST_ENTRIES + 20) {
            std::fs::write(dir.join(format!("f{i:04}.txt")), "x").unwrap();
        }
        let guard = PathGuard::new(dir.clone(), vec![]);
        let mut out = Vec::new();
        list_recursive(&guard, &dir, 1, &mut out);
        assert_eq!(out.len(), MAX_LIST_ENTRIES);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
