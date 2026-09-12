import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface Todo {
  id: string;
  title: string;
  description?: string | null;
  deadline?: string | null;
  priority: string;
  status: string;
  category: string;
  notes?: string | null;
  recurrence: string;
  completed_at?: number | null;
  created_at: number;
  updated_at: number;
}

export interface DocFolder {
  id: string;
  name: string;
  parent_id?: string | null;
  created_at: number;
  updated_at: number;
}

export interface DocumentRow {
  id: string;
  folder_id?: string | null;
  title: string;
  format: string;
  content: string;
  pinned: boolean;
  created_at: number;
  updated_at: number;
}

export interface ResearchSession {
  id: string;
  conversation_id: string;
  title: string;
  question: string;
  summary: string;
  findings: string[];
  sources: string[];
  details: string;
  open_questions: string;
  next_steps: string;
  notes: string;
  created_at: number;
  updated_at: number;
}

export interface StudySubject {
  id: string;
  name: string;
  color?: string | null;
  created_at: number;
  updated_at: number;
}

export interface StudyTopic {
  id: string;
  subject_id: string;
  name: string;
  status: string;
  last_studied?: string | null;
  deadline?: string | null;
  priority: string;
  remaining_estimate?: number | null;
  notes?: string | null;
  created_at: number;
  updated_at: number;
}

export interface TopicWithRisk {
  topic: StudyTopic;
  risk: string;
  reason: string;
}

export interface StudyAssignment {
  id: string;
  subject_id: string;
  topic_id?: string | null;
  title: string;
  kind: string;
  status: string;
  deadline?: string | null;
  priority: string;
  notes?: string | null;
  created_at: number;
  updated_at: number;
}

export interface StudySessionRow {
  id: string;
  subject_id?: string | null;
  topic_id?: string | null;
  date: string;
  duration_minutes: number;
  notes?: string | null;
  created_at: number;
}

export interface FoodEntry {
  id: string;
  name: string;
  quantity: number;
  unit: string;
  calories: number;
  protein: number;
  carbs: number;
  fat: number;
  date: string;
  meal_type: string;
  created_at: number;
}

export interface FridgeItem {
  id: string;
  name: string;
  quantity: number;
  unit: string;
  category: string;
  expiry_date?: string | null;
  created_at: number;
  updated_at: number;
}

export interface Recipe {
  id: string;
  name: string;
  calories: number;
  protein: number;
  carbs: number;
  fat: number;
  ingredients: string[];
  instructions: string;
  created_at: number;
}

export interface SuggestedMeal {
  name: string;
  calories: number;
  ingredients: string[];
  fridge_matches: string[];
  reason: string;
}

export interface WeightEntry {
  id: string;
  date: string;
  kg: number;
  notes?: string | null;
  created_at: number;
}

export interface Climb {
  id: string;
  name: string;
  grade: string;
  date: string;
  location?: string | null;
  attempts: number;
  sent: boolean;
  project: boolean;
  style?: string | null;
  notes?: string | null;
  created_at: number;
}

export interface WorkoutSet {
  id: string;
  workout_id: string;
  exercise: string;
  set_index: number;
  reps?: number | null;
  weight?: number | null;
  duration_seconds?: number | null;
  rest_seconds?: number | null;
  distance?: number | null;
  notes?: string | null;
}

export interface Workout {
  id: string;
  name: string;
  date: string;
  notes?: string | null;
  duration_minutes?: number | null;
  created_at: number;
  sets: WorkoutSet[];
}

export interface FitnessPr {
  id: string;
  exercise: string;
  metric: string;
  value: number;
  unit: string;
  date: string;
  workout_id?: string | null;
  notes?: string | null;
  created_at: number;
}

export interface FitnessOverview {
  date: string;
  calorie_target: number;
  calories_eaten: number;
  remaining: number;
  protein: number;
  carbs: number;
  fat: number;
  workouts_this_week: number;
  current_weight?: number | null;
  starting_weight?: number | null;
  weight_history: WeightEntry[];
}

export interface MoneyEntry {
  id: string;
  kind: string;
  date: string;
  description: string;
  category: string;
  amount_cents: number;
  year: number;
  month: number;
  created_at: number;
  updated_at: number;
}

export interface MoneyPot {
  id: string;
  name: string;
  slug: string;
  balance_cents: number;
  created_at: number;
  updated_at: number;
}

export interface MoneyPotShare {
  id: string;
  name: string;
  balance_cents: number;
  pct: number;
}

export interface MoneySummary {
  year: number;
  month: number;
  income_cents: number;
  expense_cents: number;
  net_cents: number;
  savings_cents: number;
  by_category: { category: string; kind: string; amount_cents: number }[];
  pots?: MoneyPotShare[];
  pots_cents?: number;
}

export interface MoneyAnalysis {
  summary: MoneySummary;
  previous?: MoneySummary | null;
  biggest_expenses: MoneyEntry[];
  recurring: string[];
  notes: string[];
}

export interface SocialProfile {
  narrative: string;
  tone_notes: string;
  updated_at: number;
}

export interface SocialThread {
  id: string;
  name: string;
  current_chapter: string;
  sort_order: number;
  updated_at: number;
}

export interface SocialProject {
  id: string;
  name: string;
  status: string;
  notes: string;
  created_at: number;
  updated_at: number;
}

export interface SocialIdea {
  id: string;
  body: string;
  platform?: string | null;
  thread_id?: string | null;
  used_at?: number | null;
  created_at: number;
  updated_at: number;
}

export interface SocialDraft {
  id: string;
  title: string;
  body: string;
  platform: string;
  thread_id?: string | null;
  archived?: boolean;
  source_post_id?: string | null;
  created_at: number;
  updated_at: number;
}

export interface SocialPost {
  id: string;
  plan_id: string;
  platform: string;
  slot_date: string;
  slot_time: string;
  category: string;
  thread_id?: string | null;
  purpose: string;
  body: string;
  suggested_media: string;
  gather: string[];
  status: string;
  calendar_event_id?: string | null;
  metrics: Record<string, unknown>;
  created_at: number;
  updated_at: number;
}

export interface SocialWeeklyPlan {
  id: string;
  week_start: string;
  status: string;
  last_week_notes: string;
  context_digest: string;
  results_json: string;
  created_at: number;
  updated_at: number;
  posts: SocialPost[];
}

export interface SnapshotEvent {
  title: string;
  start_time: number;
  all_day?: boolean;
}

export interface SnapshotTodo {
  title: string;
  deadline?: string | null;
  overdue?: boolean;
}

export interface CalorieDay {
  date: string;
  label: string;
  kcal: number;
}

export interface TaskDay {
  date: string;
  label: string;
  titles: string[];
}

export interface StudyFocus {
  name: string;
  status: string;
  deadline?: string | null;
  last_studied?: string | null;
  remaining_estimate?: number | null;
  risk?: string;
  reason?: string;
}

export interface LifeSnapshot {
  open_todos: number;
  overdue_todos: number;
  todos_completed?: number;
  calories_eaten: number;
  calorie_target: number;
  calorie_week?: CalorieDay[];
  task_week?: TaskDay[];
  week_label?: string;
  workouts_this_week?: number;
  money_net_cents?: number | null;
  money_income_cents?: number | null;
  money_expense_cents?: number | null;
  study?: string | null;
  study_subjects?: number;
  study_open?: number;
  study_completed?: number;
  study_focus?: StudyFocus | null;
  sparks_active?: number;
  docs_count?: number;
  events_today?: number;
  today_events?: SnapshotEvent[];
  open_todo_preview?: SnapshotTodo[];
  social_drafts?: number;
}

export const todoList = (status?: string | null, category?: string | null) =>
  invoke<Todo[]>("todo_list", { status: status ?? null, category: category ?? null });
export const todoUpsert = (input: Partial<Todo> & { title: string }) =>
  invoke<Todo>("todo_upsert", { input });
export const todoComplete = (id: string) => invoke<Todo[]>("todo_complete", { id });
export const todoDelete = (id: string) => invoke<void>("todo_delete", { id });

export const docsListFolders = () => invoke<DocFolder[]>("docs_list_folders");
export const docsUpsertFolder = (name: string, id?: string | null, parentId?: string | null) =>
  invoke<DocFolder>("docs_upsert_folder", { id: id ?? null, name, parentId: parentId ?? null });
export const docsDeleteFolder = (id: string) => invoke<void>("docs_delete_folder", { id });
export const docsList = (folderId?: string | null) =>
  invoke<DocumentRow[]>("docs_list", { folderId: folderId ?? null });
export const docsGet = (id: string) => invoke<DocumentRow>("docs_get", { id });
export const docsUpsert = (doc: {
  id?: string | null;
  folderId?: string | null;
  title: string;
  format: string;
  content: string;
  pinned?: boolean | null;
}) =>
  invoke<DocumentRow>("docs_upsert", {
    id: doc.id ?? null,
    folderId: doc.folderId ?? null,
    title: doc.title,
    format: doc.format,
    content: doc.content,
    pinned: doc.pinned ?? null,
  });
export const docsDelete = (id: string) => invoke<void>("docs_delete", { id });
export const docsSearch = (query: string) =>
  invoke<{ id: string; title: string; snippet: string; format: string; updated_at: number }[]>(
    "docs_search",
    { query, limit: 20 },
  );

export const createResearchConversation = (title?: string) =>
  invoke<{ id: string; title: string; created_at: number; updated_at: number; kind?: string }>(
    "create_research_conversation",
    { title: title ?? null },
  );
export const researchList = () => invoke<ResearchSession[]>("research_list");
export const researchGet = (conversationId: string) =>
  invoke<ResearchSession | null>("research_get", { conversationId });
export const researchEnsure = (conversationId: string, title: string, question: string) =>
  invoke<ResearchSession>("research_ensure", { conversationId, title, question });
export const researchUpdate = (
  conversationId: string,
  input: Partial<ResearchSession>,
) => invoke<ResearchSession>("research_update", { conversationId, input });
export const researchSaveToDoc = (conversationId: string) =>
  invoke<DocumentRow>("research_save_to_doc", { conversationId });

export const studyListSubjects = () => invoke<StudySubject[]>("study_list_subjects");
export const studyUpsertSubject = (name: string, id?: string | null, color?: string | null) =>
  invoke<StudySubject>("study_upsert_subject", { id: id ?? null, name, color: color ?? null });
export const studyDeleteSubject = (id: string) => invoke<void>("study_delete_subject", { id });
export const studyListTopics = (subjectId?: string | null) =>
  invoke<TopicWithRisk[]>("study_list_topics", { subjectId: subjectId ?? null });
export const studyUpsertTopic = (topic: StudyTopic) =>
  invoke<StudyTopic>("study_upsert_topic", { topic });
export const studyDeleteTopic = (id: string) => invoke<void>("study_delete_topic", { id });
export const studyListAssignments = (subjectId?: string | null) =>
  invoke<StudyAssignment[]>("study_list_assignments", { subjectId: subjectId ?? null });
export const studyUpsertAssignment = (assignment: StudyAssignment) =>
  invoke<StudyAssignment>("study_upsert_assignment", { assignment });
export const studyDeleteAssignment = (id: string) => invoke<void>("study_delete_assignment", { id });
export const studyListSessions = () => invoke<StudySessionRow[]>("study_list_sessions");
export const studyLogSession = (session: Partial<StudySessionRow> & { duration_minutes: number; date: string }) =>
  invoke<StudySessionRow>("study_log_session", { session });

export const fitnessOverview = () => invoke<FitnessOverview>("fitness_overview");
export const fitnessListFood = (date?: string | null) =>
  invoke<FoodEntry[]>("fitness_list_food", { date: date ?? null });
export const fitnessUpsertFood = (entry: FoodEntry) => invoke<FoodEntry>("fitness_upsert_food", { entry });
export const fitnessDeleteFood = (id: string) => invoke<void>("fitness_delete_food", { id });
export const fitnessListFridge = () => invoke<FridgeItem[]>("fitness_list_fridge");
export const fitnessUpsertFridge = (item: FridgeItem) => invoke<FridgeItem>("fitness_upsert_fridge", { item });
export const fitnessDeleteFridge = (id: string) => invoke<void>("fitness_delete_fridge", { id });
export const fitnessListRecipes = () => invoke<Recipe[]>("fitness_list_recipes");
export const fitnessSuggestMeals = () => invoke<SuggestedMeal[]>("fitness_suggest_meals");
export const fitnessListWeight = () => invoke<WeightEntry[]>("fitness_list_weight");
export const fitnessUpsertWeight = (entry: WeightEntry) => invoke<WeightEntry>("fitness_upsert_weight", { entry });
export const fitnessDeleteWeight = (id: string) => invoke<void>("fitness_delete_weight", { id });
export const fitnessListClimbs = () => invoke<Climb[]>("fitness_list_climbs");
export const fitnessUpsertClimb = (climb: Climb) => invoke<Climb>("fitness_upsert_climb", { climb });
export const fitnessDeleteClimb = (id: string) => invoke<void>("fitness_delete_climb", { id });
export const fitnessListWorkouts = () => invoke<Workout[]>("fitness_list_workouts");
export const fitnessSaveWorkout = (workout: Workout) =>
  invoke<{ workout: Workout; prs: { exercise: string; metric: string; value: number; unit: string; previous?: number | null }[] }>(
    "fitness_save_workout",
    { workout },
  );
export const fitnessDeleteWorkout = (id: string) => invoke<void>("fitness_delete_workout", { id });
export const fitnessListPrs = () => invoke<FitnessPr[]>("fitness_list_prs");

export const moneyList = (year: number, month: number) =>
  invoke<MoneyEntry[]>("money_list", { year, month });
export const moneyUpsert = (entry: MoneyEntry) => invoke<MoneyEntry>("money_upsert", { entry });
export const moneyDelete = (id: string) => invoke<void>("money_delete", { id });
export const moneySummary = (year: number, month: number) =>
  invoke<MoneySummary>("money_summary", { year, month });
export const moneyAnalyze = (year: number, month: number) =>
  invoke<MoneyAnalysis>("money_analyze", { year, month });
export const moneyListPots = () => invoke<MoneyPot[]>("money_list_pots");
export const moneyUpsertPot = (pot: MoneyPot) => invoke<MoneyPot>("money_upsert_pot", { pot });
export const moneyDeletePot = (id: string) => invoke<void>("money_delete_pot", { id });

export const socialsProfile = () => invoke<SocialProfile>("socials_profile");
export const socialsUpdateProfile = (narrative: string, toneNotes: string) =>
  invoke<SocialProfile>("socials_update_profile", { narrative, toneNotes });
export const socialsListThreads = () => invoke<SocialThread[]>("socials_list_threads");
export const socialsUpdateThread = (id: string, chapter: string) =>
  invoke<SocialThread>("socials_update_thread", { id, chapter });
export const socialsListProjects = () => invoke<SocialProject[]>("socials_list_projects");
export const socialsUpsertProject = (project: SocialProject) =>
  invoke<SocialProject>("socials_upsert_project", { project });
export const socialsDeleteProject = (id: string) => invoke<void>("socials_delete_project", { id });
export const socialsListIdeas = () => invoke<SocialIdea[]>("socials_list_ideas");
export const socialsUpsertIdea = (idea: SocialIdea) => invoke<SocialIdea>("socials_upsert_idea", { idea });
export const socialsDeleteIdea = (id: string) => invoke<void>("socials_delete_idea", { id });
export const socialsListDrafts = () => invoke<SocialDraft[]>("socials_list_drafts");
export const socialsUpsertDraft = (draft: SocialDraft) => invoke<SocialDraft>("socials_upsert_draft", { draft });
export const socialsDeleteDraft = (id: string) => invoke<void>("socials_delete_draft", { id });
export const socialsGetPlan = (weekStart?: string | null) =>
  invoke<SocialWeeklyPlan | null>("socials_get_plan", { weekStart: weekStart ?? null });
export const socialsStartReview = (weekStart?: string | null, lastWeekNotes?: string | null) =>
  invoke<SocialWeeklyPlan>("socials_start_review", {
    weekStart: weekStart ?? null,
    lastWeekNotes: lastWeekNotes ?? null,
  });
export const socialsGeneratePosts = (
  weekStart?: string | null,
  notes?: string | null,
  remake = false,
) =>
  invoke<SocialWeeklyPlan>("socials_generate_posts", {
    weekStart: weekStart ?? null,
    notes: notes ?? null,
    remake,
  });
export const socialsArchivePost = (id: string) => invoke<SocialDraft>("socials_archive_post", { id });
export const socialsUpdatePost = (post: SocialPost) => invoke<SocialPost>("socials_update_post", { post });
export const socialsCommitApproved = (planId: string) =>
  invoke<{ created: number }>("socials_commit_approved", { planId });
export const socialsMarkPublished = (id: string, metrics?: Record<string, unknown> | null) =>
  invoke<SocialPost>("socials_mark_published", { id, metrics: metrics ?? null });
export const socialsPublished = () => invoke<SocialPost[]>("socials_published");
export const socialsGetByEvent = (eventId: string) =>
  invoke<SocialPost | null>("socials_get_by_event", { eventId });

export const lifeDashboardSnapshot = () => invoke<LifeSnapshot>("life_dashboard_snapshot");
export const lifeDashboardWeek = (offset: number) =>
  invoke<{ task_week: TaskDay[]; week_label: string }>("life_dashboard_week", { offset });

export function subscribeLifeEvent(event: string, onUpdate: () => void) {
  let unsub = () => {};
  listen(event, () => onUpdate()).then((u) => {
    unsub = u;
  });
  return () => unsub();
}
