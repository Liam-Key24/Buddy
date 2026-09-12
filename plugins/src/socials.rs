use std::sync::Arc;

use buddy_calendar::{CalendarService, CreateEventInput};
use buddy_core::{
    parse_tool_json, AfterExecute, AskKind, BuddyPlugin, FieldSpec, Tool, ToolDecl, ToolError,
    ToolRegistry, ToolResult, ToolSchema, ToolSpec,
};
use buddy_database::{
    format_social_event_notes, local_today, post_event_title, week_commencing_monday, Database,
};
use chrono::{Local, NaiveDate, NaiveTime, TimeZone};
use serde::Deserialize;

pub struct SocialsPlugin;

struct WeeklyReviewTool { db: Arc<Database> }
struct GetPlanTool { db: Arc<Database> }
struct LookTool { db: Arc<Database> }
struct UpdatePostTool { db: Arc<Database> }
struct MarkPublishedTool { db: Arc<Database> }
struct SummaryTool { db: Arc<Database> }

pub struct SocialsCommitTool {
    db: Arc<Database>,
    calendar: Arc<CalendarService>,
}

const SOCIALS_SPECS: &[ToolSpec] = &[
            ToolSpec::basic("socials.weekly_review", "gather grounded context and create social slots", r#"socials.weekly_review"#, buddy_core::empty_schema("socials.weekly_review")),
            ToolSpec::basic("socials.get_plan", "fetch a weekly social plan", r#"socials.get_plan"#, buddy_core::empty_schema("socials.get_plan")),
            ToolSpec::basic("socials.look", "read socials rows", r#"socials.look what=ideas"#, buddy_core::empty_schema("socials.look")),
            ToolSpec::basic("socials.update_post", "edit/approve/reject one proposed post", r#"socials.update_post id=<id> status=approved"#, buddy_core::empty_schema("socials.update_post")),
            ToolSpec::basic("socials.commit_approved", "pin approved posts onto the Calendar", r#"socials.commit_approved plan_id=<id>"#, buddy_core::empty_schema("socials.commit_approved")),
            ToolSpec::basic("socials.mark_published", "mark a post as published", r#"socials.mark_published id=<id>"#, buddy_core::empty_schema("socials.mark_published")),
            ToolSpec::basic("socials.summary", "recent plan vs published counts", r#"socials.summary"#, buddy_core::empty_schema("socials.summary")),
        ];

impl BuddyPlugin for SocialsPlugin {
    fn id(&self) -> &'static str { "socials" }

    fn tools(&self, db: Arc<Database>) -> Vec<Arc<dyn Tool>> {
        vec![
            Arc::new(WeeklyReviewTool { db: db.clone() }),
            Arc::new(GetPlanTool { db: db.clone() }),
            Arc::new(LookTool { db: db.clone() }),
            Arc::new(UpdatePostTool { db: db.clone() }),
            Arc::new(MarkPublishedTool { db: db.clone() }),
            Arc::new(SummaryTool { db }),
        ]
    }

    fn tool_decls(&self) -> &'static [ToolDecl] {
        &[
            ToolDecl { name: "socials.weekly_review", planner_line: "socials.weekly_review: gather grounded context (story, active projects, unused ideas) and create LinkedIn(3)+X(35) slots. The Socials Weekly Review Generate/Remake buttons fill bodies with the model. Never invent GitHub posts. tool_input JSON: {\"week_start?\":\"YYYY-MM-DD\", \"last_week_notes?\":\"built/learned/struggled/shipped\"}." },
            ToolDecl { name: "socials.get_plan", planner_line: "socials.get_plan: fetch a weekly social plan. tool_input JSON: {\"week_start?\":\"YYYY-MM-DD\", \"plan_id?\":\"\"}" },
            ToolDecl { name: "socials.look", planner_line: "socials.look: read socials rows. tool_input JSON: {\"what\":\"ideas|drafts|threads|projects|published|plans|profile\"}. Use when they ask what ideas, drafts, or published posts they have." },
            ToolDecl { name: "socials.update_post", planner_line: "socials.update_post: edit/approve/reject one proposed post. tool_input JSON: {\"id\":\"...\", \"body?\":\"\", \"status?\":\"proposed|approved|rejected\", \"category?\":\"\", \"purpose?\":\"\", \"suggested_media?\":\"\", \"gather?\":[\"...\"], \"slot_time?\":\"HH:MM\"}. Do not set github platform." },
            ToolDecl { name: "socials.commit_approved", planner_line: "socials.commit_approved: pin approved posts onto the existing Calendar (not a second calendar). tool_input JSON: {\"plan_id\":\"...\"}" },
            ToolDecl { name: "socials.mark_published", planner_line: "socials.mark_published: mark a post as published after copy-paste. tool_input JSON: {\"id\":\"...\", \"metrics?\":{\"views\":0,\"likes\":0}}" },
            ToolDecl { name: "socials.summary", planner_line: "socials.summary: recent plan vs published counts and story threads. tool_input JSON: {}" },
        ]
    }

    fn tool_specs(&self) -> &'static [ToolSpec] {
        SOCIALS_SPECS
    }

    fn tool_schemas(&self) -> &'static [ToolSchema] {
        &[
            ToolSchema {
                tool: "socials.update_post",
                fields: &[FieldSpec { name: "id", label: "post id", required: true, memory_keys: &[], ask_kind: AskKind::Text, choices: &[] }],
            },
            ToolSchema {
                tool: "socials.commit_approved",
                fields: &[FieldSpec { name: "plan_id", label: "plan id", required: true, memory_keys: &[], ask_kind: AskKind::Text, choices: &[] }],
            },
        ]
    }

    fn after_execute_hint(&self, tool_name: &str) -> AfterExecute {
        match tool_name {
            "socials.weekly_review" | "socials.update_post" | "socials.commit_approved"
            | "socials.mark_published" => AfterExecute::EmitSocialsUpdated,
            _ => AfterExecute::None,
        }
    }
}

impl SocialsPlugin {
    pub fn install(registry: &mut ToolRegistry, db: Arc<Database>, calendar: Arc<CalendarService>) {
        registry.register(Arc::new(SocialsCommitTool { db, calendar }));
    }
}

#[derive(Deserialize, Default)]
struct ReviewIn {
    #[serde(default)] week_start: Option<String>,
    #[serde(default)] last_week_notes: Option<String>,
}

impl Tool for WeeklyReviewTool {
    fn name(&self) -> &str { "socials.weekly_review" }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: ReviewIn = parse_tool_json(input, "socials.weekly_review").unwrap_or_default();
        let today = local_today();
        let week = parsed.week_start.filter(|s| !s.is_empty()).unwrap_or_else(|| week_commencing_monday(&today));
        let mut context = self.db.gather_social_context().map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        if let Some(notes) = parsed.last_week_notes.as_deref() {
            context.push_str(&format!("\nUser last-week notes:\n{notes}\n"));
        }
        if let Some(t) = self.db.format_open_todos_context() {
            context.push_str(&format!("\n{t}\n"));
        }
        if let Some(study) = self.db.format_study_digest() {
            context.push_str(&format!("\n{study}\n"));
        }
        if let Some(research) = self.db.format_research_digest() {
            context.push_str(&format!("\n{research}\n"));
        }
        let plan = self.db.create_week_plan(&week, parsed.last_week_notes.as_deref().unwrap_or(""), &context)
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        let github = plan.posts.iter().any(|p| p.platform == "github");
        Ok(ToolResult {
            output: serde_json::to_string_pretty(&serde_json::json!({
                "plan_id": plan.id,
                "week_start": plan.week_start,
                "linkedin_slots": plan.posts.iter().filter(|p| p.platform=="linkedin").count(),
                "x_slots": plan.posts.iter().filter(|p| p.platform=="x").count(),
                "github_slots": plan.posts.iter().filter(|p| p.platform=="github").count(),
                "includes_github": github,
                "context": context,
                "instruction": "Fill only grounded posts via socials.update_post. Skip GitHub. Do not add posts to Calendar until the user approves, then socials.commit_approved."
            })).unwrap_or_default(),
        })
    }
}

#[derive(Deserialize, Default)]
struct GetIn {
    #[serde(default)] week_start: Option<String>,
    #[serde(default)] plan_id: Option<String>,
}

impl Tool for GetPlanTool {
    fn name(&self) -> &str { "socials.get_plan" }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: GetIn = parse_tool_json(input, "socials.get_plan").unwrap_or_default();
        let plan = if let Some(id) = parsed.plan_id.filter(|s| !s.is_empty()) {
            self.db
                .get_social_plan(&id)
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?
        } else {
            let week = parsed
                .week_start
                .unwrap_or_else(|| week_commencing_monday(&local_today()));
            self.db
                .get_social_plan_by_week(&week)
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?
                .ok_or_else(|| ToolError::ExecutionFailed(format!("no plan for {week}")))?
        };
        Ok(ToolResult { output: serde_json::to_string_pretty(&plan).unwrap_or_default() })
    }
}

#[derive(Deserialize, Default)]
struct LookIn {
    #[serde(default)]
    what: Option<String>,
}

impl Tool for LookTool {
    fn name(&self) -> &str { "socials.look" }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: LookIn = parse_tool_json(input, "socials.look").unwrap_or_default();
        let what = parsed.what.as_deref().unwrap_or("ideas").trim().to_ascii_lowercase();
        let output = match what.as_str() {
            "drafts" => serde_json::to_string_pretty(
                &self.db.list_social_drafts().map_err(|e| ToolError::ExecutionFailed(e.to_string()))?,
            ),
            "threads" => serde_json::to_string_pretty(
                &self.db.list_social_threads().map_err(|e| ToolError::ExecutionFailed(e.to_string()))?,
            ),
            "projects" => serde_json::to_string_pretty(
                &self.db.list_social_projects().map_err(|e| ToolError::ExecutionFailed(e.to_string()))?,
            ),
            "published" => serde_json::to_string_pretty(
                &self.db.list_published_posts(30).map_err(|e| ToolError::ExecutionFailed(e.to_string()))?,
            ),
            "plans" => {
                let plans = self.db.list_social_plans().map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
                let rows: Vec<_> = plans.iter().map(|p| serde_json::json!({
                    "id": p.id,
                    "week_start": p.week_start,
                    "status": p.status,
                    "linkedin": p.posts.iter().filter(|x| x.platform == "linkedin").count(),
                    "x": p.posts.iter().filter(|x| x.platform == "x").count(),
                    "published": p.posts.iter().filter(|x| x.status == "published").count(),
                })).collect();
                serde_json::to_string_pretty(&rows)
            }
            "profile" => serde_json::to_string_pretty(
                &self.db.get_social_profile().map_err(|e| ToolError::ExecutionFailed(e.to_string()))?,
            ),
            _ => serde_json::to_string_pretty(
                &self.db.list_social_ideas().map_err(|e| ToolError::ExecutionFailed(e.to_string()))?,
            ),
        }.unwrap_or_default();
        Ok(ToolResult { output })
    }
}

#[derive(Deserialize)]
struct UpdateIn {
    id: String,
    #[serde(default)] body: Option<String>,
    #[serde(default)] status: Option<String>,
    #[serde(default)] category: Option<String>,
    #[serde(default)] purpose: Option<String>,
    #[serde(default)] suggested_media: Option<String>,
    #[serde(default)] gather: Option<Vec<String>>,
    #[serde(default)] slot_time: Option<String>,
    #[serde(default)] thread_id: Option<String>,
}

impl Tool for UpdatePostTool {
    fn name(&self) -> &str { "socials.update_post" }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: UpdateIn = parse_tool_json(input, "socials.update_post")?;
        let mut post = self.db.get_social_post(&parsed.id).map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        if post.platform == "github" {
            return Err(ToolError::ExecutionFailed("GitHub has no active content plan.".into()));
        }
        if let Some(v) = parsed.body { post.body = v; }
        if let Some(v) = parsed.status { post.status = v; }
        if let Some(v) = parsed.category { post.category = v; }
        if let Some(v) = parsed.purpose { post.purpose = v; }
        if let Some(v) = parsed.suggested_media { post.suggested_media = v; }
        if let Some(v) = parsed.gather { post.gather = v; }
        if let Some(v) = parsed.slot_time { post.slot_time = v; }
        if let Some(v) = parsed.thread_id { post.thread_id = Some(v); }
        let saved = self.db.update_social_post(post).map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        Ok(ToolResult { output: serde_json::to_string_pretty(&saved).unwrap_or_default() })
    }
}

#[derive(Deserialize)]
struct CommitIn { plan_id: String }

impl Tool for SocialsCommitTool {
    fn name(&self) -> &str { "socials.commit_approved" }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: CommitIn = parse_tool_json(input, "socials.commit_approved")?;
        let plan = self.db.get_social_plan(&parsed.plan_id).map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        let threads = self.db.list_social_threads().unwrap_or_default();
        let mut created = Vec::new();
        let mut skipped = Vec::new();
        for mut post in plan.posts {
            if post.platform == "github" {
                skipped.push(post.id);
                continue;
            }
            if post.status != "approved" || post.calendar_event_id.is_some() {
                skipped.push(post.id);
                continue;
            }
            let Some((start, end)) = slot_to_ms(&post.slot_date, &post.slot_time) else {
                skipped.push(post.id);
                continue;
            };
            let thread_name = post.thread_id.as_ref().and_then(|id| threads.iter().find(|t| &t.id == id).map(|t| t.name.as_str()));
            let event = block_on(self.calendar.create_event(CreateEventInput {
                title: post_event_title(&post),
                description: Some(format_social_event_notes(&post, thread_name)),
                location: None,
                category: Some("social".into()),
                color: None,
                start_time: start,
                end_time: end,
                all_day: false,
                timezone: None,
                recurrence: None,
                reminders: vec![],
                flexibility: None,
                priority: None,
                force: true,
            }))?;
            post.calendar_event_id = Some(event.id.clone());
            let saved = self.db.update_social_post(post).map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
            created.push(saved);
        }
        let _ = self.db.update_plan_status(&parsed.plan_id, "committed", None);
        Ok(ToolResult {
            output: serde_json::to_string_pretty(&serde_json::json!({
                "created": created.len(),
                "skipped": skipped.len(),
                "events": created.iter().map(|p| serde_json::json!({"post_id": p.id, "event_id": p.calendar_event_id})).collect::<Vec<_>>(),
            })).unwrap_or_default(),
        })
    }
}

#[derive(Deserialize)]
struct PublishIn {
    id: String,
    #[serde(default)] metrics: Option<serde_json::Value>,
}

impl Tool for MarkPublishedTool {
    fn name(&self) -> &str { "socials.mark_published" }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: PublishIn = parse_tool_json(input, "socials.mark_published")?;
        let mut post = self.db.get_social_post(&parsed.id).map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        post.status = "published".into();
        if let Some(m) = parsed.metrics { post.metrics = m; }
        let saved = self.db.update_social_post(post).map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        Ok(ToolResult { output: serde_json::to_string_pretty(&saved).unwrap_or_default() })
    }
}

impl Tool for SummaryTool {
    fn name(&self) -> &str { "socials.summary" }
    fn execute(&self, _input: &str) -> Result<ToolResult, ToolError> {
        let digest = self.db.format_socials_digest().unwrap_or_default();
        let context = self.db.gather_social_context().map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        Ok(ToolResult { output: format!("{digest}\n\n{context}") })
    }
}

fn slot_to_ms(date: &str, time: &str) -> Option<(i64, i64)> {
    let d = NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()?;
    let t = NaiveTime::parse_from_str(time, "%H:%M").ok()?;
    let ndt = d.and_time(t);
    let start = Local.from_local_datetime(&ndt).single()?.timestamp_millis();
    Some((start, start + 30 * 60 * 1000))
}

fn block_on<F, T>(fut: F) -> Result<T, ToolError>
where
    F: std::future::Future<Output = Result<T, buddy_calendar::CalendarError>>,
{
    let result = match tokio::runtime::Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(fut)),
        Err(_) => tokio::runtime::Runtime::new()
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?
            .block_on(fut),
    };
    result.map_err(|e| ToolError::ExecutionFailed(format!("{}: {}", e.code(), e)))
}
