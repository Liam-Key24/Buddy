use std::sync::Arc;

use buddy_calendar::CreateEventInput;
use buddy_database::{
    format_social_event_notes, local_today, post_event_title, study_deadline_risk, suggest_meals,
    week_commencing_monday, Climb, Document, FoodEntry, FridgeItem, MoneyEntry, MoneyPot, Recipe,
    ResearchSession, SocialDraft, SocialIdea, SocialPost, SocialProject, StudyAssignment,
    StudySession, StudySubject, StudyTopic, Todo, UpdateResearchInput, UpsertTodo, WeightEntry,
    Workout,
};
use tauri::{Emitter, State};

use crate::state::AppState;

fn emit(app: &tauri::AppHandle, event: &str) {
    let _ = app.emit(event, ());
}

#[tauri::command]
pub fn todo_list(
    state: State<'_, Arc<AppState>>,
    status: Option<String>,
    category: Option<String>,
) -> Result<Vec<Todo>, String> {
    state
        .db
        .list_todos(status.as_deref(), category.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn todo_upsert(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    input: UpsertTodo,
) -> Result<Todo, String> {
    let todo = state.db.upsert_todo(input).map_err(|e| e.to_string())?;
    emit(&app, "todos-updated");
    Ok(todo)
}

#[tauri::command]
pub fn todo_complete(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<Vec<Todo>, String> {
    let todos = state.db.complete_todo(&id).map_err(|e| e.to_string())?;
    emit(&app, "todos-updated");
    Ok(todos)
}

#[tauri::command]
pub fn todo_delete(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), String> {
    state.db.delete_todo(&id).map_err(|e| e.to_string())?;
    emit(&app, "todos-updated");
    Ok(())
}

#[tauri::command]
pub fn docs_list_folders(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<buddy_database::DocFolder>, String> {
    state.db.list_doc_folders().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn docs_upsert_folder(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    id: Option<String>,
    name: String,
    parent_id: Option<String>,
) -> Result<buddy_database::DocFolder, String> {
    let f = state
        .db
        .upsert_doc_folder(id, &name, parent_id)
        .map_err(|e| e.to_string())?;
    emit(&app, "docs-updated");
    Ok(f)
}

#[tauri::command]
pub fn docs_delete_folder(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), String> {
    state.db.delete_doc_folder(&id).map_err(|e| e.to_string())?;
    emit(&app, "docs-updated");
    Ok(())
}

#[tauri::command]
pub fn docs_list(
    state: State<'_, Arc<AppState>>,
    folder_id: Option<String>,
) -> Result<Vec<Document>, String> {
    state
        .db
        .list_documents(folder_id.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn docs_get(state: State<'_, Arc<AppState>>, id: String) -> Result<Document, String> {
    state.db.get_document(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn docs_upsert(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    id: Option<String>,
    folder_id: Option<String>,
    title: String,
    format: String,
    content: String,
    pinned: Option<bool>,
) -> Result<Document, String> {
    let doc = state
        .db
        .upsert_document(id, folder_id, &title, &format, &content, pinned)
        .map_err(|e| e.to_string())?;
    emit(&app, "docs-updated");
    Ok(doc)
}

#[tauri::command]
pub fn docs_delete(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), String> {
    state.db.delete_document(&id).map_err(|e| e.to_string())?;
    emit(&app, "docs-updated");
    Ok(())
}

#[tauri::command]
pub fn docs_search(
    state: State<'_, Arc<AppState>>,
    query: String,
    limit: Option<i64>,
) -> Result<Vec<buddy_database::DocumentHit>, String> {
    state
        .db
        .search_documents(&query, limit.unwrap_or(20))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_research_conversation(
    state: State<'_, Arc<AppState>>,
    title: Option<String>,
) -> Result<buddy_database::Conversation, String> {
    let title = title.unwrap_or_else(|| "Research".into());
    let conv = state
        .db
        .create_conversation_with_kind(&title, "research", None, None)
        .map_err(|e| e.to_string())?;
    let _ = state
        .db
        .ensure_research_session(&conv.id, &title, "");
    Ok(conv)
}

#[tauri::command]
pub fn research_list(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<ResearchSession>, String> {
    state.db.list_research_sessions().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn research_get(
    state: State<'_, Arc<AppState>>,
    conversation_id: String,
) -> Result<Option<ResearchSession>, String> {
    state
        .db
        .get_research_by_conversation(&conversation_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn research_ensure(
    state: State<'_, Arc<AppState>>,
    conversation_id: String,
    title: String,
    question: String,
) -> Result<ResearchSession, String> {
    state
        .db
        .ensure_research_session(&conversation_id, &title, &question)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn research_update(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    conversation_id: String,
    input: UpdateResearchInput,
) -> Result<ResearchSession, String> {
    let _ = state
        .db
        .ensure_research_session(&conversation_id, "", "");
    let s = state
        .db
        .update_research_session(&conversation_id, input)
        .map_err(|e| e.to_string())?;
    emit(&app, "research-updated");
    Ok(s)
}

#[tauri::command]
pub fn research_save_to_doc(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    conversation_id: String,
) -> Result<Document, String> {
    let session = state
        .db
        .get_research_by_conversation(&conversation_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "research session not found".to_string())?;
    let mut md = format!("# {}\n\n", session.title);
    md.push_str(&format!("## Question\n\n{}\n\n", session.question));
    md.push_str(&format!("## Summary\n\n{}\n\n", session.summary));
    if !session.findings.is_empty() {
        md.push_str("## Key Findings\n\n");
        for f in &session.findings {
            md.push_str(&format!("- {f}\n"));
        }
        md.push('\n');
    }
    if !session.sources.is_empty() {
        md.push_str("## Sources\n\n");
        for s in &session.sources {
            md.push_str(&format!("- {s}\n"));
        }
        md.push('\n');
    }
    if !session.details.is_empty() {
        md.push_str(&format!("## Important Details\n\n{}\n\n", session.details));
    }
    if !session.open_questions.is_empty() {
        md.push_str(&format!("## Open Questions\n\n{}\n\n", session.open_questions));
    }
    if !session.next_steps.is_empty() {
        md.push_str(&format!("## Next Steps\n\n{}\n\n", session.next_steps));
    }
    if !session.notes.is_empty() {
        md.push_str(&format!("## Notes\n\n{}\n\n", session.notes));
    }
    let (format, content) = buddy_database::prepare_document_content("markdown", &md);
    let doc = state
        .db
        .upsert_document(None, None, &session.title, &format, &content, Some(false))
        .map_err(|e| e.to_string())?;
    emit(&app, "docs-updated");
    Ok(doc)
}

#[tauri::command]
pub fn study_list_subjects(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<StudySubject>, String> {
    state.db.list_study_subjects().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn study_upsert_subject(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    id: Option<String>,
    name: String,
    color: Option<String>,
) -> Result<StudySubject, String> {
    let s = state
        .db
        .upsert_study_subject(id, &name, color)
        .map_err(|e| e.to_string())?;
    emit(&app, "study-updated");
    Ok(s)
}

#[tauri::command]
pub fn study_delete_subject(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), String> {
    state.db.delete_study_subject(&id).map_err(|e| e.to_string())?;
    emit(&app, "study-updated");
    Ok(())
}

#[tauri::command]
pub fn study_list_topics(
    state: State<'_, Arc<AppState>>,
    subject_id: Option<String>,
) -> Result<Vec<serde_json::Value>, String> {
    let today = local_today();
    let pace = state.db.recent_study_minutes_per_day(14).unwrap_or(0.0);
    let planned = state.db.planned_study_minutes_ahead(14).unwrap_or(0.0);
    let topics = state
        .db
        .list_study_topics(subject_id.as_deref())
        .map_err(|e| e.to_string())?;
    Ok(topics
        .into_iter()
        .map(|t| {
            let risk = study_deadline_risk(
                &t.status,
                t.deadline.as_deref(),
                t.remaining_estimate,
                pace,
                planned,
                &t.priority,
                &today,
            );
            serde_json::json!({
                "topic": t,
                "risk": risk.status,
                "reason": risk.reason,
            })
        })
        .collect())
}

#[tauri::command]
pub fn study_upsert_topic(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    topic: StudyTopic,
) -> Result<StudyTopic, String> {
    let t = state.db.upsert_study_topic(topic).map_err(|e| e.to_string())?;
    emit(&app, "study-updated");
    Ok(t)
}

#[tauri::command]
pub fn study_delete_topic(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), String> {
    state.db.delete_study_topic(&id).map_err(|e| e.to_string())?;
    emit(&app, "study-updated");
    Ok(())
}

#[tauri::command]
pub fn study_list_assignments(
    state: State<'_, Arc<AppState>>,
    subject_id: Option<String>,
) -> Result<Vec<StudyAssignment>, String> {
    state
        .db
        .list_study_assignments(subject_id.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn study_upsert_assignment(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    assignment: StudyAssignment,
) -> Result<StudyAssignment, String> {
    let a = state
        .db
        .upsert_study_assignment(assignment)
        .map_err(|e| e.to_string())?;
    emit(&app, "study-updated");
    Ok(a)
}

#[tauri::command]
pub fn study_delete_assignment(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), String> {
    state
        .db
        .delete_study_assignment(&id)
        .map_err(|e| e.to_string())?;
    emit(&app, "study-updated");
    Ok(())
}

#[tauri::command]
pub fn study_list_sessions(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<StudySession>, String> {
    state.db.list_study_sessions(50).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn study_log_session(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    session: StudySession,
) -> Result<StudySession, String> {
    let s = state
        .db
        .log_study_session(session)
        .map_err(|e| e.to_string())?;
    emit(&app, "study-updated");
    Ok(s)
}

#[tauri::command]
pub fn fitness_overview(state: State<'_, Arc<AppState>>) -> Result<serde_json::Value, String> {
    let today = local_today();
    let target: f64 = state
        .db
        .get_setting_or("fitness_calorie_target", "2500")
        .parse()
        .unwrap_or(2500.0);
    let foods = state.db.list_food_entries(&today).unwrap_or_default();
    let eaten: f64 = foods.iter().map(|f| f.calories).sum();
    let protein: f64 = foods.iter().map(|f| f.protein).sum();
    let carbs: f64 = foods.iter().map(|f| f.carbs).sum();
    let fat: f64 = foods.iter().map(|f| f.fat).sum();
    let workouts = state.db.workouts_this_week().unwrap_or(0);
    let weights = state.db.list_weight_entries().unwrap_or_default();
    Ok(serde_json::json!({
        "date": today,
        "calorie_target": target,
        "calories_eaten": eaten,
        "remaining": target - eaten,
        "protein": protein,
        "carbs": carbs,
        "fat": fat,
        "workouts_this_week": workouts,
        "current_weight": weights.first().map(|w| w.kg),
        "starting_weight": weights.last().map(|w| w.kg),
        "weight_history": weights,
    }))
}

#[tauri::command]
pub fn fitness_list_food(
    state: State<'_, Arc<AppState>>,
    date: Option<String>,
) -> Result<Vec<FoodEntry>, String> {
    match date.as_deref().filter(|d| !d.is_empty()) {
        Some(d) => state.db.list_food_entries(d).map_err(|e| e.to_string()),
        None => state.db.list_all_food_entries().map_err(|e| e.to_string()),
    }
}

#[tauri::command]
pub fn fitness_upsert_food(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    entry: FoodEntry,
) -> Result<FoodEntry, String> {
    let e = state.db.upsert_food_entry(entry).map_err(|e| e.to_string())?;
    emit(&app, "fitness-updated");
    Ok(e)
}

#[tauri::command]
pub fn fitness_delete_food(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), String> {
    state.db.delete_food_entry(&id).map_err(|e| e.to_string())?;
    emit(&app, "fitness-updated");
    Ok(())
}

#[tauri::command]
pub fn fitness_list_fridge(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<FridgeItem>, String> {
    state.db.list_fridge().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn fitness_upsert_fridge(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    item: FridgeItem,
) -> Result<FridgeItem, String> {
    let i = state.db.upsert_fridge_item(item).map_err(|e| e.to_string())?;
    emit(&app, "fitness-updated");
    Ok(i)
}

#[tauri::command]
pub fn fitness_delete_fridge(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), String> {
    state.db.delete_fridge_item(&id).map_err(|e| e.to_string())?;
    emit(&app, "fitness-updated");
    Ok(())
}

#[tauri::command]
pub fn fitness_list_recipes(state: State<'_, Arc<AppState>>) -> Result<Vec<Recipe>, String> {
    state.db.list_recipes().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn fitness_suggest_meals(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<buddy_database::SuggestedMeal>, String> {
    let today = local_today();
    let target: f64 = state
        .db
        .get_setting_or("fitness_calorie_target", "2500")
        .parse()
        .unwrap_or(2500.0);
    let eaten: f64 = state
        .db
        .list_food_entries(&today)
        .unwrap_or_default()
        .iter()
        .map(|f| f.calories)
        .sum();
    let recipes = state.db.list_recipes().unwrap_or_default();
    let fridge = state.db.list_fridge().unwrap_or_default();
    Ok(suggest_meals(&recipes, &fridge, target - eaten))
}

#[tauri::command]
pub fn fitness_list_weight(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<WeightEntry>, String> {
    state.db.list_weight_entries().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn fitness_upsert_weight(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    entry: WeightEntry,
) -> Result<WeightEntry, String> {
    let e = state.db.upsert_weight_entry(entry).map_err(|e| e.to_string())?;
    emit(&app, "fitness-updated");
    Ok(e)
}

#[tauri::command]
pub fn fitness_delete_weight(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), String> {
    state.db.delete_weight_entry(&id).map_err(|e| e.to_string())?;
    emit(&app, "fitness-updated");
    Ok(())
}

#[tauri::command]
pub fn fitness_list_climbs(state: State<'_, Arc<AppState>>) -> Result<Vec<Climb>, String> {
    state.db.list_climbs().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn fitness_upsert_climb(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    climb: Climb,
) -> Result<Climb, String> {
    let c = state.db.upsert_climb(climb).map_err(|e| e.to_string())?;
    emit(&app, "fitness-updated");
    Ok(c)
}

#[tauri::command]
pub fn fitness_delete_climb(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), String> {
    state.db.delete_climb(&id).map_err(|e| e.to_string())?;
    emit(&app, "fitness-updated");
    Ok(())
}

#[tauri::command]
pub fn fitness_list_workouts(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<Workout>, String> {
    state.db.list_workouts(40).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn fitness_save_workout(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    workout: Workout,
) -> Result<serde_json::Value, String> {
    let (w, prs) = state.db.save_workout(workout).map_err(|e| e.to_string())?;
    emit(&app, "fitness-updated");
    Ok(serde_json::json!({ "workout": w, "prs": prs.iter().map(|p| {
        serde_json::json!({"exercise": p.exercise, "metric": p.metric, "value": p.value, "unit": p.unit, "previous": p.previous})
    }).collect::<Vec<_>>() }))
}

#[tauri::command]
pub fn fitness_delete_workout(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), String> {
    state.db.delete_workout(&id).map_err(|e| e.to_string())?;
    emit(&app, "fitness-updated");
    Ok(())
}

#[tauri::command]
pub fn fitness_list_prs(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<buddy_database::FitnessPr>, String> {
    state.db.list_prs().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn money_list(
    state: State<'_, Arc<AppState>>,
    year: i32,
    month: i32,
) -> Result<Vec<MoneyEntry>, String> {
    state
        .db
        .list_money_entries(year, month)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn money_upsert(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    entry: MoneyEntry,
) -> Result<MoneyEntry, String> {
    let e = state.db.upsert_money_entry(entry).map_err(|e| e.to_string())?;
    emit(&app, "money-updated");
    Ok(e)
}

#[tauri::command]
pub fn money_delete(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), String> {
    state.db.delete_money_entry(&id).map_err(|e| e.to_string())?;
    emit(&app, "money-updated");
    Ok(())
}

#[tauri::command]
pub fn money_summary(
    state: State<'_, Arc<AppState>>,
    year: i32,
    month: i32,
) -> Result<buddy_database::MoneyMonthSummary, String> {
    state
        .db
        .money_month_summary(year, month)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn money_analyze(
    state: State<'_, Arc<AppState>>,
    year: i32,
    month: i32,
) -> Result<buddy_database::MoneyAnalysis, String> {
    state.db.analyze_money(year, month).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn money_list_pots(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<MoneyPot>, String> {
    state.db.list_money_pots().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn money_upsert_pot(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    pot: MoneyPot,
) -> Result<MoneyPot, String> {
    let p = state.db.upsert_money_pot(pot).map_err(|e| e.to_string())?;
    emit(&app, "money-updated");
    Ok(p)
}

#[tauri::command]
pub fn money_delete_pot(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), String> {
    state.db.delete_money_pot(&id).map_err(|e| e.to_string())?;
    emit(&app, "money-updated");
    Ok(())
}

#[tauri::command]
pub fn socials_profile(
    state: State<'_, Arc<AppState>>,
) -> Result<buddy_database::SocialProfile, String> {
    state.db.get_social_profile().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn socials_update_profile(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    narrative: String,
    tone_notes: String,
) -> Result<buddy_database::SocialProfile, String> {
    let p = state
        .db
        .update_social_profile(&narrative, &tone_notes)
        .map_err(|e| e.to_string())?;
    emit(&app, "socials-updated");
    Ok(p)
}

#[tauri::command]
pub fn socials_list_threads(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<buddy_database::SocialThread>, String> {
    state.db.list_social_threads().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn socials_update_thread(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
    chapter: String,
) -> Result<buddy_database::SocialThread, String> {
    let t = state
        .db
        .update_social_thread(&id, &chapter)
        .map_err(|e| e.to_string())?;
    emit(&app, "socials-updated");
    Ok(t)
}

#[tauri::command]
pub fn socials_list_projects(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<SocialProject>, String> {
    state.db.list_social_projects().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn socials_upsert_project(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    project: SocialProject,
) -> Result<SocialProject, String> {
    let p = state
        .db
        .upsert_social_project(project)
        .map_err(|e| e.to_string())?;
    emit(&app, "socials-updated");
    Ok(p)
}

#[tauri::command]
pub fn socials_delete_project(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), String> {
    state.db.delete_social_project(&id).map_err(|e| e.to_string())?;
    emit(&app, "socials-updated");
    Ok(())
}

#[tauri::command]
pub fn socials_list_ideas(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<SocialIdea>, String> {
    state.db.list_social_ideas().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn socials_upsert_idea(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    idea: SocialIdea,
) -> Result<SocialIdea, String> {
    let i = state.db.upsert_social_idea(idea).map_err(|e| e.to_string())?;
    emit(&app, "socials-updated");
    Ok(i)
}

#[tauri::command]
pub fn socials_delete_idea(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), String> {
    state.db.delete_social_idea(&id).map_err(|e| e.to_string())?;
    emit(&app, "socials-updated");
    Ok(())
}

#[tauri::command]
pub fn socials_list_drafts(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<SocialDraft>, String> {
    state.db.list_social_drafts().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn socials_upsert_draft(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    draft: SocialDraft,
) -> Result<SocialDraft, String> {
    let d = state.db.upsert_social_draft(draft).map_err(|e| e.to_string())?;
    emit(&app, "socials-updated");
    Ok(d)
}

#[tauri::command]
pub fn socials_delete_draft(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), String> {
    state.db.delete_social_draft(&id).map_err(|e| e.to_string())?;
    emit(&app, "socials-updated");
    Ok(())
}

#[tauri::command]
pub fn socials_get_plan(
    state: State<'_, Arc<AppState>>,
    week_start: Option<String>,
) -> Result<Option<buddy_database::SocialWeeklyPlan>, String> {
    let week = week_start.unwrap_or_else(|| week_commencing_monday(&local_today()));
    state
        .db
        .get_social_plan_by_week(&week)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn socials_start_review(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    week_start: Option<String>,
    last_week_notes: Option<String>,
) -> Result<buddy_database::SocialWeeklyPlan, String> {
    let week = week_start.unwrap_or_else(|| week_commencing_monday(&local_today()));
    let mut context = state.db.gather_social_context().unwrap_or_default();
    if let Some(t) = state.db.format_open_todos_context() {
        context.push_str(&format!("\n{t}\n"));
    }
    if let Some(s) = state.db.format_study_digest() {
        context.push_str(&format!("\n{s}\n"));
    }
    if let Some(r) = state.db.format_research_digest() {
        context.push_str(&format!("\n{r}\n"));
    }
    let plan = state
        .db
        .create_week_plan(&week, last_week_notes.as_deref().unwrap_or(""), &context)
        .map_err(|e| e.to_string())?;
    emit(&app, "socials-updated");
    Ok(plan)
}

#[tauri::command]
pub async fn socials_generate_posts(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    week_start: Option<String>,
    notes: Option<String>,
    remake: bool,
) -> Result<buddy_database::SocialWeeklyPlan, String> {
    crate::socials_generate::generate_week(&app, &*state, week_start, notes, remake).await
}

#[tauri::command]
pub fn socials_archive_post(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<SocialDraft, String> {
    let post = state.db.get_social_post(&id).map_err(|e| e.to_string())?;
    if post.platform == "github" {
        return Err("GitHub has no active content plan.".into());
    }
    let draft = state
        .db
        .archive_social_post_as_draft(&post)
        .map_err(|e| e.to_string())?;
    emit(&app, "socials-updated");
    Ok(draft)
}

#[tauri::command]
pub fn socials_update_post(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    post: SocialPost,
) -> Result<SocialPost, String> {
    if post.platform == "github" {
        return Err("GitHub has no active content plan.".into());
    }
    let p = state.db.update_social_post(post).map_err(|e| e.to_string())?;
    emit(&app, "socials-updated");
    Ok(p)
}

#[tauri::command]
pub async fn socials_commit_approved(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    plan_id: String,
) -> Result<serde_json::Value, String> {
    let plan = state.db.get_social_plan(&plan_id).map_err(|e| e.to_string())?;
    let threads = state.db.list_social_threads().unwrap_or_default();
    let mut created = 0u32;
    for mut post in plan.posts {
        if post.platform == "github" || post.status != "approved" || post.calendar_event_id.is_some()
        {
            continue;
        }
        let Some((start, end)) = slot_ms(&post.slot_date, &post.slot_time) else {
            continue;
        };
        let thread_name = post.thread_id.as_ref().and_then(|id| {
            threads.iter().find(|t| &t.id == id).map(|t| t.name.as_str())
        });
        let event = state
            .calendar
            .create_event(CreateEventInput {
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
            })
            .await
            .map_err(|e| e.to_string())?;
        post.calendar_event_id = Some(event.id);
        let _ = state.db.update_social_post(post);
        created += 1;
    }
    let _ = state.db.update_plan_status(&plan_id, "committed", None);
    emit(&app, "socials-updated");
    emit(&app, "calendar-updated");
    Ok(serde_json::json!({ "created": created }))
}

#[tauri::command]
pub fn socials_mark_published(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
    metrics: Option<serde_json::Value>,
) -> Result<SocialPost, String> {
    let mut post = state.db.get_social_post(&id).map_err(|e| e.to_string())?;
    post.status = "published".into();
    if let Some(m) = metrics {
        post.metrics = m;
    }
    let p = state.db.update_social_post(post).map_err(|e| e.to_string())?;
    emit(&app, "socials-updated");
    Ok(p)
}

#[tauri::command]
pub fn socials_published(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<SocialPost>, String> {
    state.db.list_published_posts(50).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn socials_get_by_event(
    state: State<'_, Arc<AppState>>,
    event_id: String,
) -> Result<Option<SocialPost>, String> {
    state
        .db
        .get_social_post_by_event(&event_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn life_dashboard_snapshot(
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let today = local_today();
    let todos = state.db.list_todos(None, None).unwrap_or_default();
    let open = todos.iter().filter(|t| t.status != "completed").count();
    let overdue = todos
        .iter()
        .filter(|t| buddy_database::todo_is_overdue(t, &today))
        .count();
    let target: f64 = state
        .db
        .get_setting_or("fitness_calorie_target", "2500")
        .parse()
        .unwrap_or(2500.0);
    let eaten: f64 = state
        .db
        .list_food_entries(&today)
        .unwrap_or_default()
        .iter()
        .map(|f| f.calories)
        .sum();
    let parts: Vec<i32> = today.split('-').filter_map(|p| p.parse().ok()).collect();
    let money = if parts.len() >= 2 {
        state.db.money_month_summary(parts[0], parts[1]).ok()
    } else {
        None
    };
    let study = state.db.format_study_digest();
    let sparks_active = state
        .db
        .list_sparks(Some("active"))
        .map(|s| s.len())
        .unwrap_or(0);
    let docs_count = state
        .db
        .list_documents(None)
        .map(|d| d.len())
        .unwrap_or(0);
    let subjects = state.db.list_study_subjects().unwrap_or_default();
    let assignments = state
        .db
        .list_study_assignments(None)
        .unwrap_or_default();
    let study_open = assignments
        .iter()
        .filter(|a| a.status != "completed")
        .count();
    let study_completed = assignments
        .iter()
        .filter(|a| a.status == "completed")
        .count();
    let todos_completed = todos.iter().filter(|t| t.status == "completed").count();
    let workouts_this_week = state.db.workouts_this_week().unwrap_or(0);
    let social_drafts = state
        .db
        .list_social_drafts()
        .map(|d| d.len())
        .unwrap_or(0);
    let today_events = day_bounds_ms(&today)
        .and_then(|(start, end)| state.db.list_buddy_calendar_events(start, end).ok())
        .unwrap_or_default();
    let events_today = today_events.len();
    let today_events_json: Vec<serde_json::Value> = today_events
        .iter()
        .take(5)
        .map(|e| {
            serde_json::json!({
                "title": e.title,
                "start_time": e.start_time,
                "all_day": e.all_day,
            })
        })
        .collect();
    let mut open_todo_rows: Vec<&buddy_database::Todo> = todos
        .iter()
        .filter(|t| t.status != "completed")
        .collect();
    open_todo_rows.sort_by_key(|t| {
        (
            !buddy_database::todo_is_overdue(t, &today),
            t.deadline.clone().unwrap_or_else(|| "9999".into()),
        )
    });
    let open_todo_json: Vec<serde_json::Value> = open_todo_rows
        .into_iter()
        .take(5)
        .map(|t| {
            serde_json::json!({
                "title": t.title,
                "deadline": t.deadline,
                "overdue": buddy_database::todo_is_overdue(t, &today),
            })
        })
        .collect();
    let pace = state.db.recent_study_minutes_per_day(14).unwrap_or(0.0);
    let planned = state.db.planned_study_minutes_ahead(14).unwrap_or(0.0);
    let topics = state.db.list_study_topics(None).unwrap_or_default();
    let study_focus = topics
        .iter()
        .filter(|t| t.status != "completed")
        .max_by_key(|t| t.updated_at)
        .map(|t| {
            let risk = study_deadline_risk(
                &t.status,
                t.deadline.as_deref(),
                t.remaining_estimate,
                pace,
                planned,
                &t.priority,
                &today,
            );
            serde_json::json!({
                "name": t.name,
                "status": t.status,
                "deadline": t.deadline,
                "last_studied": t.last_studied,
                "remaining_estimate": t.remaining_estimate,
                "risk": risk.status,
                "reason": risk.reason,
            })
        });
    Ok(serde_json::json!({
        "open_todos": open,
        "overdue_todos": overdue,
        "todos_completed": todos_completed,
        "calories_eaten": eaten,
        "calorie_target": target,
        "calorie_week": calorie_week(&state.db, &today),
        "task_week": task_week(&state.db, &todos, &today),
        "week_label": week_label(&today),
        "workouts_this_week": workouts_this_week,
        "money_net_cents": money.as_ref().map(|m| m.net_cents),
        "money_income_cents": money.as_ref().map(|m| m.income_cents),
        "money_expense_cents": money.as_ref().map(|m| m.expense_cents),
        "study": study,
        "study_subjects": subjects.len(),
        "study_open": study_open,
        "study_completed": study_completed,
        "study_focus": study_focus,
        "sparks_active": sparks_active,
        "docs_count": docs_count,
        "events_today": events_today,
        "today_events": today_events_json,
        "open_todo_preview": open_todo_json,
        "social_drafts": social_drafts,
    }))
}

#[tauri::command]
pub fn life_dashboard_week(
    state: State<'_, Arc<AppState>>,
    offset: i32,
) -> Result<serde_json::Value, String> {
    let today = local_today();
    let anchor = shift_week(&today, offset).unwrap_or(today);
    let todos = state.db.list_todos(None, None).unwrap_or_default();
    Ok(serde_json::json!({
        "task_week": task_week(&state.db, &todos, &anchor),
        "week_label": week_label(&anchor),
    }))
}

fn shift_week(today: &str, offset: i32) -> Option<String> {
    use chrono::{Duration, NaiveDate};
    let d = NaiveDate::parse_from_str(today, "%Y-%m-%d").ok()?;
    Some((d + Duration::days(i64::from(offset) * 7)).format("%Y-%m-%d").to_string())
}

fn week_label(today: &str) -> String {
    use chrono::{Datelike, NaiveDate};
    let Ok(d) = NaiveDate::parse_from_str(today, "%Y-%m-%d") else {
        return today.to_string();
    };
    format!("{} {} week", d.day(), d.format("%b"))
}

fn is_sleep_item(title: &str, category: &str) -> bool {
    title.eq_ignore_ascii_case("sleep") || category.eq_ignore_ascii_case("sleep")
}

fn work_weekdays(db: &buddy_database::Database) -> std::collections::HashSet<String> {
    let mut days = std::collections::HashSet::new();
    let Ok(rules) = db.list_lifestyle_schedule_rules() else {
        return days;
    };
    for rule in rules {
        if !rule.kind.eq_ignore_ascii_case("work") {
            continue;
        }
        let Ok(segs) = serde_json::from_str::<Vec<serde_json::Value>>(&rule.segments_json) else {
            continue;
        };
        for seg in segs {
            let arr = seg
                .get("by_day")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            if arr.is_empty() {
                for code in ["MO", "TU", "WE", "TH", "FR"] {
                    days.insert(code.into());
                }
                continue;
            }
            for day in arr {
                if let Some(code) = day.as_str() {
                    days.insert(code.to_ascii_uppercase());
                }
            }
        }
    }
    days
}

fn weekday_code(d: chrono::Weekday) -> &'static str {
    use chrono::Weekday::*;
    match d {
        Mon => "MO",
        Tue => "TU",
        Wed => "WE",
        Thu => "TH",
        Fri => "FR",
        Sat => "SA",
        Sun => "SU",
    }
}

fn local_date_from_ms(ms: i64) -> Option<String> {
    use chrono::Local;
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|dt| dt.with_timezone(&Local).format("%Y-%m-%d").to_string())
}

fn task_week(
    db: &buddy_database::Database,
    todos: &[buddy_database::Todo],
    today: &str,
) -> Vec<serde_json::Value> {
    use chrono::{Datelike, Duration, NaiveDate};
    let Ok(anchor) = NaiveDate::parse_from_str(today, "%Y-%m-%d") else {
        return Vec::new();
    };
    let monday = anchor - Duration::days(anchor.weekday().num_days_from_monday() as i64);
    let sunday = monday + Duration::days(6);
    let week_start = day_bounds_ms(&monday.format("%Y-%m-%d").to_string()).map(|(s, _)| s);
    let week_end = day_bounds_ms(&sunday.format("%Y-%m-%d").to_string()).map(|(_, e)| e);
    let events = match (week_start, week_end) {
        (Some(start), Some(end)) => db
            .list_buddy_calendar_events(start, end)
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    let work_days = work_weekdays(db);
    (0..7)
        .map(|i| {
            let day = monday + Duration::days(i);
            let key = day.format("%Y-%m-%d").to_string();
            let mut titles = Vec::new();
            let mut seen = std::collections::HashSet::new();
            if work_days.contains(weekday_code(day.weekday())) && seen.insert("work".into()) {
                titles.push("Work".to_string());
            }
            for ev in &events {
                if is_sleep_item(&ev.title, &ev.category) {
                    continue;
                }
                if local_date_from_ms(ev.start_time).as_deref() != Some(key.as_str()) {
                    continue;
                }
                if seen.insert(format!("ev:{}", ev.id)) {
                    titles.push(ev.title.clone());
                }
            }
            for todo in todos {
                let due = todo.deadline.as_deref() == Some(key.as_str());
                let done = todo.status == "completed"
                    && todo
                        .completed_at
                        .and_then(local_date_from_ms)
                        .as_deref()
                        == Some(key.as_str());
                if (due || done) && seen.insert(format!("todo:{}", todo.id)) {
                    titles.push(todo.title.clone());
                }
            }
            serde_json::json!({
                "date": key,
                "label": day.format("%a").to_string(),
                "titles": titles,
            })
        })
        .collect()
}

fn calorie_week(db: &buddy_database::Database, today: &str) -> Vec<serde_json::Value> {
    use chrono::{Duration, NaiveDate};
    let Ok(anchor) = NaiveDate::parse_from_str(today, "%Y-%m-%d") else {
        return Vec::new();
    };
    (0..7)
        .rev()
        .map(|i| {
            let day = anchor - Duration::days(i);
            let key = day.format("%Y-%m-%d").to_string();
            let kcal: f64 = db
                .list_food_entries(&key)
                .unwrap_or_default()
                .iter()
                .map(|f| f.calories)
                .sum();
            serde_json::json!({
                "date": key,
                "label": day.format("%a").to_string(),
                "kcal": kcal.round() as i64,
            })
        })
        .collect()
}

fn day_bounds_ms(today: &str) -> Option<(i64, i64)> {
    use chrono::{Local, NaiveDate, TimeZone};
    let d = NaiveDate::parse_from_str(today, "%Y-%m-%d").ok()?;
    let start = Local
        .from_local_datetime(&d.and_hms_opt(0, 0, 0)?)
        .single()?
        .timestamp_millis();
    Some((start, start + 86_400_000))
}

fn slot_ms(date: &str, time: &str) -> Option<(i64, i64)> {
    use chrono::{Local, NaiveDate, NaiveTime, TimeZone};
    let d = NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()?;
    let t = NaiveTime::parse_from_str(time, "%H:%M").ok()?;
    let start = Local.from_local_datetime(&d.and_time(t)).single()?.timestamp_millis();
    Some((start, start + 30 * 60 * 1000))
}
