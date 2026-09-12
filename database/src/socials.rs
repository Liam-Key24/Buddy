use std::collections::HashMap;

use rusqlite::params;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{chrono_now, local_today, Database, DbError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialProfile {
    pub narrative: String,
    pub tone_notes: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialThread {
    pub id: String,
    pub name: String,
    pub current_chapter: String,
    pub sort_order: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialProject {
    pub id: String,
    pub name: String,
    pub status: String,
    pub notes: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialIdea {
    pub id: String,
    pub body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub used_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialDraft {
    pub id: String,
    pub title: String,
    pub body: String,
    pub platform: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default)]
    pub archived: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_post_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialWeeklyPlan {
    pub id: String,
    pub week_start: String,
    pub status: String,
    pub last_week_notes: String,
    pub context_digest: String,
    pub results_json: String,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub posts: Vec<SocialPost>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialPost {
    pub id: String,
    pub plan_id: String,
    pub platform: String,
    pub slot_date: String,
    pub slot_time: String,
    pub category: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    pub purpose: String,
    pub body: String,
    pub suggested_media: String,
    pub gather: Vec<String>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calendar_event_id: Option<String>,
    pub metrics: serde_json::Value,
    pub created_at: i64,
    pub updated_at: i64,
}

pub const DEFAULT_LI_TIMES: &[&str] = &["09:00", "09:00", "09:00"];
pub const DEFAULT_X_TIMES: &[&str] = &["08:00", "10:30", "13:00", "16:00", "20:00"];

pub fn week_commencing_monday(today: &str) -> String {
    // If Sunday, next Monday; otherwise this week's Monday (or next if we want upcoming).
    // Sunday review plans the week commencing the following Monday.
    let parts: Vec<i32> = today.split('-').filter_map(|p| p.parse().ok()).collect();
    if parts.len() != 3 {
        return today.to_string();
    }
    let ord = crate::todos::ymd_to_ordinal_pub(parts[0], parts[1], parts[2]).unwrap_or(1);
    // 1970-01-01 was Thursday. weekday: 0=Thu ...
    // Unix day 0 = Thu. We want Mon=0 style: 
    let unix_days = ord - 1; // days since 1970-01-01
    let weekday = (unix_days + 3).rem_euclid(7); // 0=Mon .. 6=Sun
    let days_until_next_monday = if weekday == 6 {
        1
    } else {
        (7 - weekday) % 7
    };
    // For Sun -> next Mon. For Mon-Sat during a week, "week commencing" for review is next Monday
    // if today is Sun; if generating mid-week, still next Monday unless today is Monday then this Monday.
    let offset = if weekday == 6 {
        1
    } else if weekday == 0 {
        0
    } else {
        7 - weekday
    };
    let _ = days_until_next_monday;
    crate::todos::add_days_to_date(today, offset as i64).unwrap_or_else(|| today.to_string())
}

pub fn format_social_event_notes(post: &SocialPost, thread_name: Option<&str>) -> String {
    let gather = if post.gather.is_empty() {
        "—".into()
    } else {
        post.gather
            .iter()
            .map(|g| format!("- {g}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        "Platform: {}\n\nStory Thread: {}\n\nPurpose: {}\n\nPost:\n\n{}\n\nSuggested Media:\n\n{}\n\nContent to Gather:\n{}",
        post.platform,
        thread_name.unwrap_or(post.thread_id.as_deref().unwrap_or("—")),
        post.purpose,
        post.body,
        post.suggested_media,
        gather
    )
}

pub fn post_event_title(post: &SocialPost) -> String {
    let platform = match post.platform.as_str() {
        "linkedin" => "LinkedIn",
        "x" => "X",
        "github" => "GitHub",
        other => other,
    };
    let label = if post.category.is_empty() {
        "Post".into()
    } else {
        let mut c = post.category.clone();
        if let Some(first) = c.get_mut(0..1) {
            first.make_ascii_uppercase();
        }
        c
    };
    format!("{platform} — {label}")
}

impl Database {
    pub fn get_social_profile(&self) -> Result<SocialProfile, DbError> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT narrative, tone_notes, updated_at FROM social_profile WHERE id='default'",
                [],
                |row| {
                    Ok(SocialProfile {
                        narrative: row.get(0)?,
                        tone_notes: row.get(1)?,
                        updated_at: row.get(2)?,
                    })
                },
            )
            .map_err(DbError::from)
        })
    }

    pub fn update_social_profile(&self, narrative: &str, tone_notes: &str) -> Result<SocialProfile, DbError> {
        let now = chrono_now();
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE social_profile SET narrative=?1, tone_notes=?2, updated_at=?3 WHERE id='default'",
                params![narrative, tone_notes, now],
            )?;
            Ok(())
        })?;
        Ok(SocialProfile {
            narrative: narrative.to_string(),
            tone_notes: tone_notes.to_string(),
            updated_at: now,
        })
    }

    pub fn list_social_threads(&self) -> Result<Vec<SocialThread>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, current_chapter, sort_order, updated_at FROM social_threads ORDER BY sort_order",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(SocialThread {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    current_chapter: row.get(2)?,
                    sort_order: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    pub fn update_social_thread(&self, id: &str, chapter: &str) -> Result<SocialThread, DbError> {
        let now = chrono_now();
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE social_threads SET current_chapter=?1, updated_at=?2 WHERE id=?3",
                params![chapter, now, id],
            )?;
            Ok(())
        })?;
        self.list_social_threads()?
            .into_iter()
            .find(|t| t.id == id)
            .ok_or_else(|| DbError::NotFound(id.into()))
    }

    pub fn list_social_projects(&self) -> Result<Vec<SocialProject>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, status, notes, created_at, updated_at FROM social_projects ORDER BY updated_at DESC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(SocialProject {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    status: row.get(2)?,
                    notes: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    pub fn upsert_social_project(&self, p: SocialProject) -> Result<SocialProject, DbError> {
        let now = chrono_now();
        let mut p = p;
        if p.id.is_empty() {
            p.id = Uuid::new_v4().to_string();
            p.created_at = now;
        }
        p.updated_at = now;
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO social_projects (id, name, status, notes, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6)
                 ON CONFLICT(id) DO UPDATE SET name=excluded.name, status=excluded.status, notes=excluded.notes, updated_at=excluded.updated_at",
                params![p.id, p.name, p.status, p.notes, p.created_at, p.updated_at],
            )?;
            Ok(())
        })?;
        Ok(p)
    }

    pub fn delete_social_project(&self, id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let n = conn.execute("DELETE FROM social_projects WHERE id=?1", params![id])?;
            if n == 0 { Err(DbError::NotFound(id.into())) } else { Ok(()) }
        })
    }

    pub fn list_social_ideas(&self) -> Result<Vec<SocialIdea>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, body, platform, thread_id, used_at, created_at, updated_at FROM social_ideas ORDER BY used_at IS NOT NULL, updated_at DESC LIMIT 500",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(SocialIdea {
                    id: row.get(0)?,
                    body: row.get(1)?,
                    platform: row.get(2)?,
                    thread_id: row.get(3)?,
                    used_at: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    pub fn list_unused_social_ideas(&self) -> Result<Vec<SocialIdea>, DbError> {
        Ok(self
            .list_social_ideas()?
            .into_iter()
            .filter(|i| i.used_at.is_none())
            .collect())
    }

    pub fn upsert_social_idea(&self, i: SocialIdea) -> Result<SocialIdea, DbError> {
        let now = chrono_now();
        let mut i = i;
        if i.id.is_empty() {
            i.id = Uuid::new_v4().to_string();
            i.created_at = now;
        }
        i.updated_at = now;
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO social_ideas (id, body, platform, thread_id, used_at, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7)
                 ON CONFLICT(id) DO UPDATE SET body=excluded.body, platform=excluded.platform, thread_id=excluded.thread_id,
                 used_at=COALESCE(excluded.used_at, social_ideas.used_at), updated_at=excluded.updated_at",
                params![i.id, i.body, i.platform, i.thread_id, i.used_at, i.created_at, i.updated_at],
            )?;
            Ok(())
        })?;
        Ok(i)
    }

    pub fn mark_social_idea_used(&self, id: &str) -> Result<(), DbError> {
        let now = chrono_now();
        self.with_conn(|conn| {
            let n = conn.execute(
                "UPDATE social_ideas SET used_at=COALESCE(used_at, ?1), updated_at=?1 WHERE id=?2",
                params![now, id],
            )?;
            if n == 0 {
                Err(DbError::NotFound(id.into()))
            } else {
                Ok(())
            }
        })
    }

    pub fn delete_social_idea(&self, id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let n = conn.execute("DELETE FROM social_ideas WHERE id=?1", params![id])?;
            if n == 0 { Err(DbError::NotFound(id.into())) } else { Ok(()) }
        })
    }

    pub fn list_social_drafts(&self) -> Result<Vec<SocialDraft>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, title, body, platform, thread_id, archived, source_post_id, created_at, updated_at FROM social_drafts ORDER BY archived ASC, updated_at DESC LIMIT 500",
            )?;
            let rows = stmt.query_map([], |row| {
                let archived_i: i64 = row.get(5)?;
                Ok(SocialDraft {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    body: row.get(2)?,
                    platform: row.get(3)?,
                    thread_id: row.get(4)?,
                    archived: archived_i != 0,
                    source_post_id: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    pub fn list_open_social_drafts(&self) -> Result<Vec<SocialDraft>, DbError> {
        Ok(self
            .list_social_drafts()?
            .into_iter()
            .filter(|d| !d.archived)
            .collect())
    }

    pub fn upsert_social_draft(&self, d: SocialDraft) -> Result<SocialDraft, DbError> {
        let now = chrono_now();
        let mut d = d;
        if d.id.is_empty() {
            d.id = Uuid::new_v4().to_string();
            d.created_at = now;
        }
        d.updated_at = now;
        let archived = if d.archived { 1i64 } else { 0 };
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO social_drafts (id, title, body, platform, thread_id, archived, source_post_id, created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)
                 ON CONFLICT(id) DO UPDATE SET title=excluded.title, body=excluded.body, platform=excluded.platform,
                 thread_id=excluded.thread_id, archived=excluded.archived, source_post_id=excluded.source_post_id, updated_at=excluded.updated_at",
                params![
                    d.id,
                    d.title,
                    d.body,
                    d.platform,
                    d.thread_id,
                    archived,
                    d.source_post_id,
                    d.created_at,
                    d.updated_at
                ],
            )?;
            Ok(())
        })?;
        Ok(d)
    }

    pub fn archive_social_post_as_draft(&self, post: &SocialPost) -> Result<SocialDraft, DbError> {
        let preview: String = post.body.chars().take(48).collect();
        let title = if preview.is_empty() {
            format!("{} {}", post.platform, post.slot_date)
        } else {
            preview
        };
        self.upsert_social_draft(SocialDraft {
            id: String::new(),
            title,
            body: post.body.clone(),
            platform: post.platform.clone(),
            thread_id: post.thread_id.clone(),
            archived: true,
            source_post_id: Some(post.id.clone()),
            created_at: 0,
            updated_at: 0,
        })
    }

    pub fn delete_social_draft(&self, id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let n = conn.execute("DELETE FROM social_drafts WHERE id=?1", params![id])?;
            if n == 0 { Err(DbError::NotFound(id.into())) } else { Ok(()) }
        })
    }

    pub fn get_social_plan_by_week(&self, week_start: &str) -> Result<Option<SocialWeeklyPlan>, DbError> {
        let plan = self.with_conn(|conn| {
            match conn.query_row(
                "SELECT id, week_start, status, last_week_notes, context_digest, results_json, created_at, updated_at FROM social_weekly_plans WHERE week_start=?1",
                params![week_start],
                map_plan,
            ) {
                Ok(p) => Ok(Some(p)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(DbError::from(e)),
            }
        })?;
        if let Some(mut p) = plan {
            p.posts = self.list_social_posts(&p.id)?;
            Ok(Some(p))
        } else {
            Ok(None)
        }
    }

    pub fn get_social_plan(&self, id: &str) -> Result<SocialWeeklyPlan, DbError> {
        let mut plan = self.with_conn(|conn| {
            conn.query_row(
                "SELECT id, week_start, status, last_week_notes, context_digest, results_json, created_at, updated_at FROM social_weekly_plans WHERE id=?1",
                params![id],
                map_plan,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => DbError::NotFound(id.into()),
                other => DbError::from(other),
            })
        })?;
        plan.posts = self.list_social_posts(&plan.id)?;
        Ok(plan)
    }

    pub fn list_social_plans(&self) -> Result<Vec<SocialWeeklyPlan>, DbError> {
        let mut plans = self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, week_start, status, last_week_notes, context_digest, results_json, created_at, updated_at FROM social_weekly_plans ORDER BY week_start DESC LIMIT 104",
            )?;
            let rows = stmt.query_map([], map_plan)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })?;
        if plans.is_empty() {
            return Ok(plans);
        }
        let mut posts_by_plan: HashMap<String, Vec<SocialPost>> = HashMap::new();
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, plan_id, platform, slot_date, slot_time, category, thread_id, purpose, body, suggested_media, gather_json, status, calendar_event_id, metrics_json, created_at, updated_at
                 FROM social_posts WHERE plan_id IN (SELECT id FROM social_weekly_plans ORDER BY week_start DESC LIMIT 104)
                 ORDER BY plan_id, slot_date, slot_time, platform",
            )?;
            let rows = stmt.query_map([], map_post)?;
            for post in rows {
                let post = post?;
                posts_by_plan.entry(post.plan_id.clone()).or_default().push(post);
            }
            Ok(())
        })?;
        for p in &mut plans {
            p.posts = posts_by_plan.remove(&p.id).unwrap_or_default();
        }
        Ok(plans)
    }

    pub fn list_social_posts(&self, plan_id: &str) -> Result<Vec<SocialPost>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, plan_id, platform, slot_date, slot_time, category, thread_id, purpose, body, suggested_media, gather_json, status, calendar_event_id, metrics_json, created_at, updated_at
                 FROM social_posts WHERE plan_id=?1 ORDER BY slot_date, slot_time, platform",
            )?;
            let rows = stmt.query_map(params![plan_id], map_post)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    pub fn get_social_post(&self, id: &str) -> Result<SocialPost, DbError> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT id, plan_id, platform, slot_date, slot_time, category, thread_id, purpose, body, suggested_media, gather_json, status, calendar_event_id, metrics_json, created_at, updated_at
                 FROM social_posts WHERE id=?1",
                params![id],
                map_post,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => DbError::NotFound(id.into()),
                other => DbError::from(other),
            })
        })
    }

    pub fn get_social_post_by_event(&self, event_id: &str) -> Result<Option<SocialPost>, DbError> {
        self.with_conn(|conn| {
            match conn.query_row(
                "SELECT id, plan_id, platform, slot_date, slot_time, category, thread_id, purpose, body, suggested_media, gather_json, status, calendar_event_id, metrics_json, created_at, updated_at
                 FROM social_posts WHERE calendar_event_id=?1",
                params![event_id],
                map_post,
            ) {
                Ok(p) => Ok(Some(p)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(DbError::from(e)),
            }
        })
    }

    pub fn create_week_plan(
        &self,
        week_start: &str,
        last_week_notes: &str,
        context_digest: &str,
    ) -> Result<SocialWeeklyPlan, DbError> {
        if let Some(existing) = self.get_social_plan_by_week(week_start)? {
            return Ok(existing);
        }
        let now = chrono_now();
        let id = Uuid::new_v4().to_string();
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO social_weekly_plans (id, week_start, status, last_week_notes, context_digest, results_json, created_at, updated_at)
                 VALUES (?1,?2,'review',?3,?4,'{}',?5,?5)",
                params![id, week_start, last_week_notes, context_digest, now],
            )?;
            Ok(())
        })?;
        let posts = scaffold_week_posts(&id, week_start, now);
        for p in &posts {
            self.insert_social_post(p)?;
        }
        self.get_social_plan(&id)
    }

    pub fn ensure_week_plan(
        &self,
        week_start: &str,
        last_week_notes: &str,
        context_digest: &str,
    ) -> Result<SocialWeeklyPlan, DbError> {
        if let Some(existing) = self.get_social_plan_by_week(week_start)? {
            self.update_plan_notes(&existing.id, last_week_notes, context_digest)?;
            return self.get_social_plan(&existing.id);
        }
        self.create_week_plan(week_start, last_week_notes, context_digest)
    }

    pub fn update_plan_notes(
        &self,
        id: &str,
        last_week_notes: &str,
        context_digest: &str,
    ) -> Result<(), DbError> {
        let now = chrono_now();
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE social_weekly_plans SET last_week_notes=?1, context_digest=?2, updated_at=?3 WHERE id=?4",
                params![last_week_notes, context_digest, now, id],
            )?;
            Ok(())
        })
    }

    pub fn clear_posts_for_remake(&self, plan_id: &str) -> Result<(), DbError> {
        let now = chrono_now();
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE social_posts SET body='', purpose='', category='', suggested_media='', gather_json='[]', status='proposed', updated_at=?1
                 WHERE plan_id=?2 AND calendar_event_id IS NULL AND status NOT IN ('published')",
                params![now, plan_id],
            )?;
            Ok(())
        })
    }

    fn insert_social_post(&self, p: &SocialPost) -> Result<(), DbError> {
        let gather = serde_json::to_string(&p.gather).unwrap_or_else(|_| "[]".into());
        let metrics = p.metrics.to_string();
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO social_posts (id, plan_id, platform, slot_date, slot_time, category, thread_id, purpose, body, suggested_media, gather_json, status, calendar_event_id, metrics_json, created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",
                params![
                    p.id, p.plan_id, p.platform, p.slot_date, p.slot_time, p.category, p.thread_id,
                    p.purpose, p.body, p.suggested_media, gather, p.status, p.calendar_event_id, metrics,
                    p.created_at, p.updated_at
                ],
            )?;
            Ok(())
        })
    }

    pub fn update_social_post(&self, post: SocialPost) -> Result<SocialPost, DbError> {
        let mut p = post;
        p.updated_at = chrono_now();
        if p.platform == "github" && p.status == "proposed" {
            p.status = "rejected".into();
        }
        let gather = serde_json::to_string(&p.gather).unwrap_or_else(|_| "[]".into());
        let metrics = p.metrics.to_string();
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE social_posts SET platform=?1, slot_date=?2, slot_time=?3, category=?4, thread_id=?5, purpose=?6, body=?7,
                 suggested_media=?8, gather_json=?9, status=?10, calendar_event_id=?11, metrics_json=?12, updated_at=?13 WHERE id=?14",
                params![
                    p.platform, p.slot_date, p.slot_time, p.category, p.thread_id, p.purpose, p.body,
                    p.suggested_media, gather, p.status, p.calendar_event_id, metrics, p.updated_at, p.id
                ],
            )?;
            Ok(())
        })?;
        Ok(p)
    }

    pub fn update_plan_status(&self, id: &str, status: &str, results_json: Option<&str>) -> Result<(), DbError> {
        let now = chrono_now();
        self.with_conn(|conn| {
            if let Some(r) = results_json {
                conn.execute(
                    "UPDATE social_weekly_plans SET status=?1, results_json=?2, updated_at=?3 WHERE id=?4",
                    params![status, r, now, id],
                )?;
            } else {
                conn.execute(
                    "UPDATE social_weekly_plans SET status=?1, updated_at=?2 WHERE id=?3",
                    params![status, now, id],
                )?;
            }
            Ok(())
        })
    }

    pub fn list_published_posts(&self, limit: i64) -> Result<Vec<SocialPost>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, plan_id, platform, slot_date, slot_time, category, thread_id, purpose, body, suggested_media, gather_json, status, calendar_event_id, metrics_json, created_at, updated_at
                 FROM social_posts WHERE status='published' ORDER BY slot_date DESC LIMIT ?1",
            )?;
            let rows = stmt.query_map(params![limit], map_post)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    pub fn gather_social_context(&self) -> Result<String, DbError> {
        let profile = self.get_social_profile()?;
        let threads = self.list_social_threads()?;
        let projects = self.list_social_projects()?;
        let ideas: Vec<_> = self.list_unused_social_ideas()?.into_iter().take(20).collect();
        let drafts: Vec<_> = self.list_open_social_drafts()?.into_iter().take(20).collect();
        let published = self.list_published_posts(12)?;
        let plans = self.list_social_plans()?;
        let last = plans.iter().find(|p| p.status == "complete" || p.status == "committed");
        let trunc = |s: &str, n: usize| {
            let t: String = s.chars().take(n).collect();
            if s.chars().count() > n {
                format!("{t}…")
            } else {
                t
            }
        };
        let mut out = String::new();
        out.push_str("## Current story\n");
        out.push_str(&format!("{}\nTone: {}\n", trunc(&profile.narrative, 400), trunc(&profile.tone_notes, 200)));
        out.push_str("## Story threads\n");
        for t in threads.iter().take(12) {
            out.push_str(&format!("- {} [{}]: {}\n", t.name, t.id, trunc(&t.current_chapter, 160)));
        }
        out.push_str("## Active projects\n");
        let active: Vec<_> = projects.iter().filter(|p| p.status == "active").take(12).collect();
        if active.is_empty() {
            out.push_str("(none)\n");
        } else {
            for p in active {
                out.push_str(&format!("- {}: {}\n", p.name, trunc(&p.notes, 160)));
            }
        }
        out.push_str("## Unused content ideas\n");
        if ideas.is_empty() {
            out.push_str("(none)\n");
        } else {
            for i in &ideas {
                let plat = i.platform.as_deref().unwrap_or("any");
                out.push_str(&format!("- [{}] ({}) {}\n", i.id, plat, trunc(&i.body, 200)));
            }
        }
        if !drafts.is_empty() {
            out.push_str("## Open drafts (not archived)\n");
            for d in &drafts {
                out.push_str(&format!("- [{}] {} / {}\n", d.id, d.title, trunc(&d.body, 200)));
            }
        }
        if let Some(plan) = last {
            let li_planned = plan.posts.iter().filter(|p| p.platform == "linkedin").count();
            let li_pub = plan.posts.iter().filter(|p| p.platform == "linkedin" && p.status == "published").count();
            let x_planned = plan.posts.iter().filter(|p| p.platform == "x").count();
            let x_pub = plan.posts.iter().filter(|p| p.platform == "x" && p.status == "published").count();
            out.push_str(&format!(
                "Previous week {} results: LinkedIn {li_pub}/{li_planned} published, X {x_pub}/{x_planned}. Notes: {}\n",
                plan.week_start, plan.last_week_notes
            ));
        }
        if !published.is_empty() {
            out.push_str("## Recent published (do not repeat)\n");
            for p in published.iter().take(8) {
                let preview: String = p.body.chars().take(80).collect();
                out.push_str(&format!("- {} {}: {preview}\n", p.slot_date, p.platform));
            }
        }
        Ok(out)
    }

    pub fn format_socials_digest(&self) -> Option<String> {
        let profile = self.get_social_profile().ok()?;
        let today = local_today();
        let week = week_commencing_monday(&today);
        let plan = self.get_social_plan_by_week(&week).ok().flatten();
        let counts = plan
            .as_ref()
            .map(|p| {
                let approved = p.posts.iter().filter(|x| x.status == "approved" || x.status == "published").count();
                let published = p.posts.iter().filter(|x| x.status == "published").count();
                format!("Week {week}: {approved} approved, {published} published.")
            })
            .unwrap_or_else(|| "No social plan this week.".into());
        Some(format!("Socials: {} {counts}", profile.narrative))
    }
}

fn map_plan(row: &rusqlite::Row<'_>) -> Result<SocialWeeklyPlan, rusqlite::Error> {
    Ok(SocialWeeklyPlan {
        id: row.get(0)?,
        week_start: row.get(1)?,
        status: row.get(2)?,
        last_week_notes: row.get(3)?,
        context_digest: row.get(4)?,
        results_json: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
        posts: vec![],
    })
}

fn map_post(row: &rusqlite::Row<'_>) -> Result<SocialPost, rusqlite::Error> {
    let gather_json: String = row.get(10)?;
    let metrics_json: String = row.get(13)?;
    Ok(SocialPost {
        id: row.get(0)?,
        plan_id: row.get(1)?,
        platform: row.get(2)?,
        slot_date: row.get(3)?,
        slot_time: row.get(4)?,
        category: row.get(5)?,
        thread_id: row.get(6)?,
        purpose: row.get(7)?,
        body: row.get(8)?,
        suggested_media: row.get(9)?,
        gather: serde_json::from_str(&gather_json).unwrap_or_default(),
        status: row.get(11)?,
        calendar_event_id: row.get(12)?,
        metrics: serde_json::from_str(&metrics_json).unwrap_or(serde_json::json!({})),
        created_at: row.get(14)?,
        updated_at: row.get(15)?,
    })
}

fn scaffold_week_posts(plan_id: &str, week_start: &str, now: i64) -> Vec<SocialPost> {
    let mut posts = Vec::new();
    let li_days = [0, 2, 4]; // Mon Wed Fri
    for (i, day) in li_days.iter().enumerate() {
        let date = crate::todos::add_days_to_date(week_start, *day).unwrap_or_else(|| week_start.to_string());
        posts.push(empty_post(
            plan_id,
            "linkedin",
            &date,
            DEFAULT_LI_TIMES[i.min(2)],
            now,
        ));
    }
    for day in 0..7 {
        let date = crate::todos::add_days_to_date(week_start, day).unwrap_or_else(|| week_start.to_string());
        for t in DEFAULT_X_TIMES {
            posts.push(empty_post(plan_id, "x", &date, t, now));
        }
    }
    posts
}

fn empty_post(plan_id: &str, platform: &str, date: &str, time: &str, now: i64) -> SocialPost {
    SocialPost {
        id: Uuid::new_v4().to_string(),
        plan_id: plan_id.to_string(),
        platform: platform.to_string(),
        slot_date: date.to_string(),
        slot_time: time.to_string(),
        category: String::new(),
        thread_id: None,
        purpose: String::new(),
        body: String::new(),
        suggested_media: String::new(),
        gather: vec![],
        status: "proposed".into(),
        calendar_event_id: None,
        metrics: serde_json::json!({}),
        created_at: now,
        updated_at: now,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn never_scaffolds_github() {
        let posts = scaffold_week_posts("p", "2026-08-10", 0);
        assert_eq!(posts.iter().filter(|p| p.platform == "linkedin").count(), 3);
        assert_eq!(posts.iter().filter(|p| p.platform == "x").count(), 35);
        assert!(!posts.iter().any(|p| p.platform == "github"));
    }

    #[test]
    fn event_notes_include_post_and_media() {
        let post = SocialPost {
            id: "1".into(),
            plan_id: "p".into(),
            platform: "linkedin".into(),
            slot_date: "2026-08-11".into(),
            slot_time: "09:00".into(),
            category: "building".into(),
            thread_id: Some("building_apps".into()),
            purpose: "Show progress".into(),
            body: "I spent the morning on auth.".into(),
            suggested_media: "Screenshot of the login screen.".into(),
            gather: vec!["Auth UI screenshot".into()],
            status: "approved".into(),
            calendar_event_id: None,
            metrics: serde_json::json!({}),
            created_at: 0,
            updated_at: 0,
        };
        let notes = format_social_event_notes(&post, Some("Building Apps"));
        assert!(notes.contains("I spent the morning on auth."));
        assert!(notes.contains("Screenshot of the login screen."));
        assert!(notes.contains("Auth UI screenshot"));
        assert_eq!(post_event_title(&post), "LinkedIn — Building");
    }

    #[test]
    fn gather_skips_used_ideas_and_archived_drafts() {
        let dir = std::env::temp_dir().join(format!("buddy-soc-ctx-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = crate::Database::open(&dir.join("buddy.db")).unwrap();
        db.update_social_profile("Building Buddy locally.", "honest, technical").unwrap();
        db.upsert_social_project(SocialProject {
            id: String::new(),
            name: "Buddy".into(),
            status: "active".into(),
            notes: "desktop assistant".into(),
            created_at: 0,
            updated_at: 0,
        })
        .unwrap();
        let unused = db
            .upsert_social_idea(SocialIdea {
                id: String::new(),
                body: "write about local models".into(),
                platform: Some("x".into()),
                thread_id: None,
                used_at: None,
                created_at: 0,
                updated_at: 0,
            })
            .unwrap();
        let used = db
            .upsert_social_idea(SocialIdea {
                id: String::new(),
                body: "already posted this idea".into(),
                platform: None,
                thread_id: None,
                used_at: None,
                created_at: 0,
                updated_at: 0,
            })
            .unwrap();
        db.mark_social_idea_used(&used.id).unwrap();
        db.upsert_social_draft(SocialDraft {
            id: String::new(),
            title: "open draft".into(),
            body: "draft body keep".into(),
            platform: "x".into(),
            thread_id: None,
            archived: false,
            source_post_id: None,
            created_at: 0,
            updated_at: 0,
        })
        .unwrap();
        db.upsert_social_draft(SocialDraft {
            id: String::new(),
            title: "old".into(),
            body: "archived draft hide".into(),
            platform: "linkedin".into(),
            thread_id: None,
            archived: true,
            source_post_id: None,
            created_at: 0,
            updated_at: 0,
        })
        .unwrap();

        let ctx = db.gather_social_context().unwrap();
        assert!(ctx.contains("Building Buddy locally."));
        assert!(ctx.contains("Buddy"));
        assert!(ctx.contains("write about local models"));
        assert!(ctx.contains(&unused.id));
        assert!(!ctx.contains("already posted this idea"));
        assert!(ctx.contains("draft body keep"));
        assert!(!ctx.contains("archived draft hide"));

        let plan = db.create_week_plan("2026-08-10", "", &ctx).unwrap();
        let draft = db.archive_social_post_as_draft(&plan.posts[0]).unwrap();
        assert!(draft.archived);
        assert_eq!(draft.source_post_id.as_deref(), Some(plan.posts[0].id.as_str()));

        let _ = std::fs::remove_dir_all(dir);
    }
}
