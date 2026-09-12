use rusqlite::params;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{chrono_now, fts_match_query, Database, DbError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocFolder {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<String>,
    pub title: String,
    pub format: String,
    pub content: String,
    pub pinned: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentHit {
    pub id: String,
    pub title: String,
    pub snippet: String,
    pub format: String,
    pub updated_at: i64,
}

impl Database {
    pub fn list_doc_folders(&self) -> Result<Vec<DocFolder>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, parent_id, created_at, updated_at FROM doc_folders ORDER BY name",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(DocFolder {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    parent_id: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    pub fn upsert_doc_folder(
        &self,
        id: Option<String>,
        name: &str,
        parent_id: Option<String>,
    ) -> Result<DocFolder, DbError> {
        let now = chrono_now();
        let id = id
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let existing = self
            .list_doc_folders()?
            .into_iter()
            .find(|f| f.id == id);
        let folder = DocFolder {
            id: id.clone(),
            name: name.trim().to_string(),
            parent_id,
            created_at: existing.map(|f| f.created_at).unwrap_or(now),
            updated_at: now,
        };
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO doc_folders (id, name, parent_id, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(id) DO UPDATE SET name=excluded.name, parent_id=excluded.parent_id, updated_at=excluded.updated_at",
                params![folder.id, folder.name, folder.parent_id, folder.created_at, folder.updated_at],
            )?;
            Ok(())
        })?;
        Ok(folder)
    }

    pub fn delete_doc_folder(&self, id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE documents SET folder_id = NULL WHERE folder_id = ?1",
                params![id],
            )?;
            let n = conn.execute("DELETE FROM doc_folders WHERE id = ?1", params![id])?;
            if n == 0 {
                return Err(DbError::NotFound(id.to_string()));
            }
            Ok(())
        })
    }

    fn map_document_row(row: &rusqlite::Row<'_>) -> Result<Document, rusqlite::Error> {
        let pinned: i64 = row.get(5)?;
        Ok(Document {
            id: row.get(0)?,
            folder_id: row.get(1)?,
            title: row.get(2)?,
            format: row.get(3)?,
            content: row.get(4)?,
            pinned: pinned != 0,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
        })
    }

    pub fn list_documents(&self, folder_id: Option<&str>) -> Result<Vec<Document>, DbError> {
        self.with_conn(|conn| {
            let sql = if folder_id.is_some() {
                "SELECT id, folder_id, title, format, '' AS content, pinned, created_at, updated_at FROM documents WHERE folder_id = ?1 ORDER BY pinned DESC, updated_at DESC LIMIT 2000"
            } else {
                "SELECT id, folder_id, title, format, '' AS content, pinned, created_at, updated_at FROM documents ORDER BY pinned DESC, updated_at DESC LIMIT 2000"
            };
            let mut stmt = conn.prepare(sql)?;
            let rows = if let Some(fid) = folder_id {
                stmt.query_map(params![fid], Self::map_document_row)?
            } else {
                stmt.query_map([], Self::map_document_row)?
            };
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    pub fn get_document(&self, id_or_title: &str) -> Result<Document, DbError> {
        let key = id_or_title.trim();
        if key.is_empty() {
            return Err(DbError::NotFound(id_or_title.to_string()));
        }
        self.with_conn(|conn| {
            if let Ok(doc) = conn.query_row(
                "SELECT id, folder_id, title, format, content, pinned, created_at, updated_at FROM documents WHERE id = ?1",
                params![key],
                Self::map_document_row,
            ) {
                return Ok(doc);
            }
            conn.query_row(
                "SELECT id, folder_id, title, format, content, pinned, created_at, updated_at FROM documents WHERE title = ?1 COLLATE NOCASE ORDER BY updated_at DESC LIMIT 1",
                params![key],
                Self::map_document_row,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => DbError::NotFound(key.to_string()),
                other => DbError::from(other),
            })
        })
    }

    pub fn upsert_document(
        &self,
        id: Option<String>,
        folder_id: Option<String>,
        title: &str,
        format: &str,
        content: &str,
        pinned: Option<bool>,
    ) -> Result<Document, DbError> {
        let now = chrono_now();
        let id = id
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let existing = self.get_document(&id).ok();
        let doc = Document {
            id: id.clone(),
            folder_id: folder_id.or_else(|| existing.as_ref().and_then(|d| d.folder_id.clone())),
            title: title.trim().to_string(),
            format: if format.is_empty() {
                existing
                    .as_ref()
                    .map(|d| d.format.clone())
                    .unwrap_or_else(|| "markdown".into())
            } else {
                format.to_string()
            },
            content: content.to_string(),
            pinned: pinned.unwrap_or(existing.as_ref().map(|d| d.pinned).unwrap_or(false)),
            created_at: existing.as_ref().map(|d| d.created_at).unwrap_or(now),
            updated_at: now,
        };
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO documents (id, folder_id, title, format, content, pinned, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(id) DO UPDATE SET
                    folder_id=excluded.folder_id, title=excluded.title, format=excluded.format,
                    content=excluded.content, pinned=excluded.pinned, updated_at=excluded.updated_at",
                params![
                    doc.id,
                    doc.folder_id,
                    doc.title,
                    doc.format,
                    doc.content,
                    if doc.pinned { 1 } else { 0 },
                    doc.created_at,
                    doc.updated_at,
                ],
            )?;
            Ok(())
        })?;
        Ok(doc)
    }

    pub fn delete_document(&self, id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let n = conn.execute("DELETE FROM documents WHERE id = ?1", params![id])?;
            if n == 0 {
                return Err(DbError::NotFound(id.to_string()));
            }
            Ok(())
        })
    }

    /// Improve heading/paragraph structure without the model rewriting the body.
    pub fn format_document(&self, id_or_title: &str) -> Result<Document, DbError> {
        let doc = self.get_document(id_or_title)?;
        let improved = improve_document_format(&doc.format, &doc.content);
        self.upsert_document(
            Some(doc.id),
            doc.folder_id.clone(),
            &doc.title,
            &doc.format,
            &improved,
            Some(doc.pinned),
        )
    }

    /// Replace one exact snippet in an existing document.
    pub fn patch_document(
        &self,
        id_or_title: &str,
        find: &str,
        replace: &str,
    ) -> Result<Document, DbError> {
        let doc = self.get_document(id_or_title)?;
        if find.is_empty() {
            return Err(DbError::NotFound("empty find".into()));
        }
        if !doc.content.contains(find) {
            return Err(DbError::NotFound(format!(
                "snippet not found in {}",
                doc.title
            )));
        }
        let next = doc.content.replacen(find, replace, 1);
        self.upsert_document(
            Some(doc.id),
            doc.folder_id.clone(),
            &doc.title,
            &doc.format,
            &next,
            Some(doc.pinned),
        )
    }

    pub fn search_documents(&self, query: &str, limit: i64) -> Result<Vec<DocumentHit>, DbError> {
        let q = query.trim();
        if q.is_empty() {
            return Ok(vec![]);
        }
        let like = format!("%{q}%");
        let fts_q = fts_match_query(q);
        self.with_conn(|conn| {
            // FTS if possible; fall back to LIKE so empty/special queries still work.
            let fts = conn.prepare(
                "SELECT d.id, d.title, snippet(documents_fts, 1, '[', ']', '…', 16), d.format, d.updated_at
                 FROM documents_fts
                 JOIN documents d ON d.rowid = documents_fts.rowid
                 WHERE documents_fts MATCH ?1
                 LIMIT ?2",
            );
            if !fts_q.is_empty() {
                if let Ok(mut stmt) = fts {
                    if let Ok(rows) = stmt.query_map(params![fts_q, limit], |row| {
                        Ok(DocumentHit {
                            id: row.get(0)?,
                            title: row.get(1)?,
                            snippet: row.get(2)?,
                            format: row.get(3)?,
                            updated_at: row.get(4)?,
                        })
                    }) {
                        if let Ok(hits) = rows.collect::<Result<Vec<_>, _>>() {
                            if !hits.is_empty() {
                                return Ok(hits);
                            }
                        }
                    }
                }
            }
            let mut stmt = conn.prepare(
                "SELECT id, title, substr(content, 1, 180), format, updated_at
                 FROM documents
                 WHERE title LIKE ?1 OR content LIKE ?1
                 ORDER BY pinned DESC, updated_at DESC
                 LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![like, limit], |row| {
                Ok(DocumentHit {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    snippet: row.get(2)?,
                    format: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }
}

pub fn content_looks_like_html(content: &str) -> bool {
    let t = content.trim_start();
    t.starts_with('<')
        && t.contains('>')
        && (t.starts_with("<h")
            || t.starts_with("<p")
            || t.starts_with("<div")
            || t.starts_with("<ul")
            || t.starts_with("<ol")
            || t.starts_with("<table")
            || t.starts_with("<pre")
            || t.starts_with("<blockquote")
            || t.starts_with("<article")
            || t.contains("</"))
}

pub fn markdown_to_html(md: &str) -> String {
    let mut opts = pulldown_cmark::Options::empty();
    opts.insert(pulldown_cmark::Options::ENABLE_TABLES);
    opts.insert(pulldown_cmark::Options::ENABLE_STRIKETHROUGH);
    opts.insert(pulldown_cmark::Options::ENABLE_TASKLISTS);
    let parser = pulldown_cmark::Parser::new_ext(md, opts);
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, parser);
    html
}

/// Collapse messy HTML/markdown into cleaner TipTap HTML (headings, lists, paragraphs).
pub fn improve_document_format(format: &str, content: &str) -> String {
    let fmt = format.trim().to_ascii_lowercase();
    if fmt == "csv" {
        return content
            .lines()
            .map(|l| l.trim_end())
            .collect::<Vec<_>>()
            .join("\n");
    }
    let md = if content_looks_like_html(content) {
        html_to_markdown_lite(content)
    } else {
        content.to_string()
    };
    let md = tidy_markdown(&md);
    prepare_document_content("markdown", &md).1
}

pub fn html_to_markdown_lite(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let lower = html.to_ascii_lowercase();
    let mut i = 0;
    let orig = html.as_bytes();
    let low = lower.as_bytes();
    while i < orig.len() {
        if low[i] == b'<' {
            if let Some(rel) = low[i..].iter().position(|&b| b == b'>') {
                let tag_slice = &lower[i + 1..i + rel];
                let is_close = tag_slice.starts_with('/');
                let name = tag_slice
                    .trim_start_matches('/')
                    .split(|c: char| c.is_whitespace() || c == '/')
                    .next()
                    .unwrap_or("");
                match (name, is_close) {
                    ("h1", false) => out.push_str("\n# "),
                    ("h2", false) => out.push_str("\n## "),
                    ("h3", false) => out.push_str("\n### "),
                    ("h4", false) => out.push_str("\n#### "),
                    ("p" | "div" | "br" | "tr" | "table" | "thead" | "tbody", _) => {
                        out.push('\n');
                    }
                    ("li", false) => out.push_str("\n- "),
                    ("ul" | "ol", true) => out.push('\n'),
                    ("strong" | "b", _) => out.push_str("**"),
                    ("em" | "i", _) => out.push('*'),
                    ("code", _) => out.push('`'),
                    ("a", false) => {}
                    _ => {}
                }
                i += rel + 1;
                continue;
            }
        }
        let ch = html[i..].chars().next().unwrap_or('\0');
        out.push(ch);
        i += ch.len_utf8();
    }
    decode_basic_entities(&out)
}

fn decode_basic_entities(s: &str) -> String {
    s.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

fn tidy_markdown(md: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut blanks = 0;
    for line in md.lines() {
        let trimmed = line.trim_end();
        if trimmed.trim().is_empty() {
            blanks += 1;
            if blanks <= 1 {
                out.push(String::new());
            }
            continue;
        }
        blanks = 0;
        out.push(trimmed.to_string());
    }
    out.join("\n").trim().to_string()
}

/// TipTap expects HTML for non-CSV docs. Pass markdown through; leave HTML/CSV as-is.
pub fn prepare_document_content(format: &str, content: &str) -> (String, String) {
    let fmt = format.trim().to_ascii_lowercase();
    if fmt == "csv" {
        return ("csv".into(), content.to_string());
    }
    if fmt == "html" || content_looks_like_html(content) {
        let html = if content.trim().is_empty() {
            "<p></p>".into()
        } else {
            content.to_string()
        };
        return ("html".into(), html);
    }
    let html = markdown_to_html(content);
    let html = if html.trim().is_empty() {
        "<p></p>".into()
    } else {
        html
    };
    ("html".into(), html)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Database;
    use std::fs;
    use uuid::Uuid;

    #[test]
    fn get_document_by_title_and_markdown_becomes_html() {
        let dir = std::env::temp_dir().join(format!("buddy-docs-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let db = Database::open(&dir.join("buddy.db")).unwrap();
        let (format, content) = prepare_document_content("markdown", "# Hello\n\nWorld");
        assert_eq!(format, "html");
        assert!(content.contains("<h1>") && content.contains("Hello"));
        let doc = db
            .upsert_document(None, None, "bello.today", &format, &content, Some(false))
            .unwrap();
        let by_title = db.get_document("bello.today").unwrap();
        assert_eq!(by_title.id, doc.id);
        assert_eq!(db.get_document(&doc.id).unwrap().title, "bello.today");
        let formatted = db.format_document("bello.today").unwrap();
        assert!(formatted.content.contains("<h1>"), "{}", formatted.content);
        let patched = db
            .patch_document("bello.today", "Hello", "Howdy")
            .unwrap();
        assert!(patched.content.contains("Howdy"), "{}", patched.content);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn improve_format_promotes_plain_html() {
        let messy = "<div>bello.today</div><div>ship notes</div><p>next.js bootstrapped</p>";
        let out = improve_document_format("html", messy);
        assert!(out.contains("<p>") || out.contains("<h"), "{out}");
        assert!(!out.contains("<div>"), "{out}");
    }
}
