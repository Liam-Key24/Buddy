export type Goal = {
  id: string;
  conversation_id: string;
  title: string;
  domain?: string | null;
  target?: string | null;
  deadline?: string | null;
  baseline?: string | null;
  frequency?: string | null;
  commitment?: string | null;
  status: string;
  facts?: Record<string, unknown>;
};

export type Category = {
  id: string;
  name: string;
  color: string;
  icon: string;
  keywords: string;
  sort_order?: number;
};

export type Session = {
  id: string;
  goal_id?: string | null;
  title: string;
  start_at: string;
  end_at: string;
  kind: string;
  status: string;
  proposal_batch_id?: string | null;
  notes?: string | null;
  category_id?: string | null;
  category?: Category | null;
};

export type Spark = {
  id: string;
  content: string;
  status: string;
  promoted_goal_id?: string | null;
  created_at: string;
};

export type ClarificationQuestion = {
  id: string;
  label: string;
  help_text?: string | null;
  answer_type: string;
  required: boolean;
  options: string[];
  suggested_answer?: string | null;
  reason?: string | null;
};

export type ActivityStep = {
  stage: string;
  label: string;
  detail?: string | null;
};

export type Conversation = {
  id: string;
  title: string;
  created_at: string;
  updated_at: string;
  deleted_at?: string | null;
  folder_id?: string | null;
  sort_order?: number;
  user_message_count?: number;
  draft?: Record<string, unknown>;
};

export type ChatFolder = {
  id: string;
  title: string;
  sort_order: number;
  created_at: string;
  updated_at: string;
};

export type ProposalSummary = {
  pattern?: string | null;
  total?: number;
  through?: string | null;
  text?: string;
  sample?: Session[];
  why?: Record<string, unknown>;
  why_lines?: string[];
  goal_card?: {
    title?: string;
    outcome?: string | null;
    baseline?: string | null;
    deadline?: string | null;
    frequency?: string | null;
    strategy?: string | null;
    realistic_note?: string;
  };
};

export type FixedBlock = {
  id: string;
  title: string;
  weekday: number;
  start_minute: number;
  end_minute: number;
};

export type OpenProposal = {
  goal: Goal | null;
  proposed_sessions: Session[];
  proposal_summary: ProposalSummary | null;
};

export type ChatResponse = {
  conversation_id: string;
  reply: string;
  goal: Goal | null;
  pending_question: string | null;
  proposed_sessions?: Session[];
  booked_sessions?: Session[];
  sparks?: Spark[];
  unresolved?: string[];
  ai_available?: boolean;
  proposal_summary?: ProposalSummary | null;
  clarification_questions?: ClarificationQuestion[];
  activity?: ActivityStep[];
  undo_batch_id?: string | null;
  request_id?: string | null;
  deleted_session_ids?: string[];
  updated_sessions?: Session[];
};

/** Notify Calendar/Today views to reload sessions after Chat changes the calendar. */
export function notifyCalendarChanged() {
  window.dispatchEvent(new CustomEvent("buddy.calendar-changed"));
}

export type TodayNeed = {
  id: string;
  kind: "gathering" | "approve";
  title: string;
  detail?: string | null;
  goal_id: string;
  conversation_id: string;
  proposal_batch_id?: string | null;
};

export type TodayResponse = {
  goals: Goal[];
  attention: string[];
  pending_questions: string[];
  needs?: TodayNeed[];
  todays_sessions: Session[];
  progress: Array<{
    goal_id: string;
    title: string;
    summary?: string;
    scheduled: number;
    completed: number;
    missed: number;
    proposed: number;
  }>;
  resurfaced_spark?: Spark | null;
};

export type UsageSummary = {
  used: number;
  limit: number;
  remaining: number;
  reset?: string | null;
  source: string;
  label: string;
};

export const API_BASE = import.meta.env.VITE_API_BASE ?? "/api";

export const APP_VERSION = "0.1.0";

export type HealthResponse = {
  ok: boolean;
  product: string;
  ai: {
    label: string;
    enabled: boolean;
    configured: boolean;
    model_configured: string;
  };
  db: string;
  host: string;
  privacy: string;
};

async function json<T>(res: Response): Promise<T> {
  if (!res.ok) {
    let detail = "";
    try {
      const body = (await res.json()) as { detail?: unknown };
      if (typeof body.detail === "string") detail = body.detail;
    } catch {
      /* ignore */
    }
    throw new Error(detail || `Request failed (${res.status})`);
  }
  return res.json();
}

export async function fetchHealth(): Promise<HealthResponse> {
  return json(await fetch(`${API_BASE}/health`));
}

export async function sendChat(
  message: string,
  conversationId?: string | null,
  signal?: AbortSignal,
  requestId?: string | null,
): Promise<ChatResponse> {
  const res = await fetch(`${API_BASE}/chat`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      message,
      conversation_id: conversationId || null,
      request_id: requestId || null,
    }),
    signal,
  });
  return json(res);
}

export async function cancelChat(requestId: string): Promise<{ ok: boolean }> {
  return json(
    await fetch(`${API_BASE}/chat/cancel`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ request_id: requestId }),
    }),
  );
}

export async function fetchOpenProposal(conversationId: string): Promise<OpenProposal> {
  return json(await fetch(`${API_BASE}/conversations/${conversationId}/open-proposal`));
}

export function buildProposalSummary(
  goal: Goal | null,
  sessions: Session[],
): ProposalSummary {
  if (!sessions.length) {
    return {
      total: 0,
      sample: [],
      text: "No sessions proposed.",
      why_lines: [],
      goal_card: goal
        ? {
            title: goal.title,
            outcome: goal.target,
            baseline: goal.baseline,
            deadline: goal.deadline,
            frequency: goal.frequency,
            strategy: goal.commitment,
          }
        : undefined,
    };
  }
  const sorted = [...sessions].sort((a, b) => a.start_at.localeCompare(b.start_at));
  const last = sorted[sorted.length - 1];
  return {
    pattern: goal?.frequency || null,
    total: sessions.length,
    through: last?.end_at?.slice(0, 10) ?? null,
    sample: sorted.slice(0, 5),
    why_lines: [],
    goal_card: goal
      ? {
          title: goal.title,
          outcome: goal.target,
          baseline: goal.baseline,
          deadline: goal.deadline,
          frequency: goal.frequency,
          strategy: goal.commitment,
        }
      : undefined,
  };
}

export async function fetchToday(): Promise<TodayResponse> {
  return json(await fetch(`${API_BASE}/today`));
}

export async function fetchSessions(): Promise<Session[]> {
  return json(await fetch(`${API_BASE}/calendar/sessions`));
}

export async function createSession(input: {
  title: string;
  start_at: string;
  end_at: string;
  category_id?: string | null;
}): Promise<Session> {
  return json(
    await fetch(`${API_BASE}/calendar/sessions`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(input),
    }),
  );
}

export async function updateSession(
  id: string,
  input: {
    title?: string;
    start_at?: string;
    end_at?: string;
    category_id?: string | null;
  },
): Promise<Session> {
  return json(
    await fetch(`${API_BASE}/calendar/sessions/${id}`, {
      method: "PATCH",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(input),
    }),
  );
}

export async function deleteSession(id: string): Promise<void> {
  await json(await fetch(`${API_BASE}/calendar/sessions/${id}`, { method: "DELETE" }));
}

export async function fetchCategories(): Promise<Category[]> {
  return json(await fetch(`${API_BASE}/categories`));
}

export async function createCategory(input: {
  name: string;
  color?: string;
  icon?: string;
  keywords?: string;
}): Promise<Category> {
  return json(
    await fetch(`${API_BASE}/categories`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(input),
    }),
  );
}

export async function updateCategory(
  id: string,
  input: Partial<Pick<Category, "name" | "color" | "icon" | "keywords">>,
): Promise<Category> {
  return json(
    await fetch(`${API_BASE}/categories/${id}`, {
      method: "PATCH",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(input),
    }),
  );
}

export async function deleteCategory(id: string): Promise<void> {
  await json(await fetch(`${API_BASE}/categories/${id}`, { method: "DELETE" }));
}

export async function fetchFixedBlocks(): Promise<FixedBlock[]> {
  return json(await fetch(`${API_BASE}/calendar/fixed`));
}

export async function createFixedBlock(input: {
  title: string;
  weekday: number;
  start_minute: number;
  end_minute: number;
}): Promise<FixedBlock> {
  return json(
    await fetch(`${API_BASE}/calendar/fixed`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(input),
    }),
  );
}

export async function updateFixedBlock(
  id: string,
  input: Partial<Pick<FixedBlock, "title" | "weekday" | "start_minute" | "end_minute">>,
): Promise<FixedBlock> {
  return json(
    await fetch(`${API_BASE}/calendar/fixed/${id}`, {
      method: "PATCH",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(input),
    }),
  );
}

export async function deleteFixedBlock(id: string): Promise<void> {
  await json(await fetch(`${API_BASE}/calendar/fixed/${id}`, { method: "DELETE" }));
}

export function formatSessionTime(session: Session): string {
  return new Date(session.start_at).toLocaleTimeString(undefined, {
    hour: "numeric",
    minute: "2-digit",
  });
}

export async function decideProposal(
  batchId: string,
  decision: "approve" | "reject" | "adjust" | "undo",
  conversationId?: string | null,
): Promise<{ booked?: Session[]; rejected?: number; undone?: Session[]; undo_batch_id?: string }> {
  return json(
    await fetch(`${API_BASE}/calendar/proposals/decide`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        batch_id: batchId,
        decision,
        conversation_id: conversationId || null,
      }),
    }),
  );
}

export async function markSessionOutcome(
  sessionId: string,
  outcome: "completed" | "missed",
): Promise<Session> {
  return json(
    await fetch(`${API_BASE}/calendar/sessions/outcome`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ session_id: sessionId, outcome }),
    }),
  );
}

export async function fetchMessages(
  conversationId: string,
): Promise<Array<{ role: string; content: string }>> {
  return json(await fetch(`${API_BASE}/conversations/${conversationId}/messages`));
}

export async function listConversations(): Promise<Conversation[]> {
  return json(await fetch(`${API_BASE}/conversations`));
}

export async function createConversation(): Promise<Conversation> {
  return json(await fetch(`${API_BASE}/conversations`, { method: "POST" }));
}

export async function renameConversation(id: string, title: string): Promise<Conversation> {
  return json(
    await fetch(`${API_BASE}/conversations/${id}`, {
      method: "PATCH",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ title }),
    }),
  );
}

export async function moveConversation(
  id: string,
  folderId: string | null,
): Promise<Conversation> {
  return json(
    await fetch(`${API_BASE}/conversations/${id}`, {
      method: "PATCH",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ folder_id: folderId }),
    }),
  );
}

export async function placeConversation(
  id: string,
  folderId: string | null,
  beforeId?: string | null,
): Promise<Conversation> {
  return json(
    await fetch(`${API_BASE}/conversations/${id}/place`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ folder_id: folderId, before_id: beforeId ?? null }),
    }),
  );
}

export async function fetchFolders(): Promise<ChatFolder[]> {
  return json(await fetch(`${API_BASE}/folders`));
}

export async function createFolder(title: string): Promise<ChatFolder> {
  return json(
    await fetch(`${API_BASE}/folders`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ title }),
    }),
  );
}

export async function renameFolder(id: string, title: string): Promise<ChatFolder> {
  return json(
    await fetch(`${API_BASE}/folders/${id}`, {
      method: "PATCH",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ title }),
    }),
  );
}

export async function deleteFolder(id: string): Promise<ChatFolder> {
  return json(await fetch(`${API_BASE}/folders/${id}`, { method: "DELETE" }));
}

export async function deleteConversation(id: string): Promise<Conversation> {
  return json(await fetch(`${API_BASE}/conversations/${id}`, { method: "DELETE" }));
}

export async function restoreConversation(id: string): Promise<Conversation> {
  return json(await fetch(`${API_BASE}/conversations/${id}/restore`, { method: "POST" }));
}

export async function saveDraft(
  id: string,
  draft: Record<string, unknown>,
): Promise<Conversation> {
  return json(
    await fetch(`${API_BASE}/conversations/${id}/draft`, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ draft }),
    }),
  );
}

export async function fetchUsage(): Promise<UsageSummary> {
  return json(await fetch(`${API_BASE}/ai/usage`));
}

export async function fetchSparks(): Promise<Spark[]> {
  return json(await fetch(`${API_BASE}/sparks`));
}

export async function createSpark(content: string): Promise<Spark> {
  return json(
    await fetch(`${API_BASE}/sparks`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ content }),
    }),
  );
}

export async function promoteSpark(
  id: string,
): Promise<{ chat: ChatResponse; spark_id: string } | Spark> {
  return json(
    await fetch(`${API_BASE}/sparks/${id}/promote`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({}),
    }),
  );
}

export async function dismissSpark(id: string): Promise<Spark> {
  return json(await fetch(`${API_BASE}/sparks/${id}/dismiss`, { method: "POST" }));
}

export async function fetchGoals(): Promise<Goal[]> {
  return json(await fetch(`${API_BASE}/goals`));
}

export async function deleteGoal(id: string): Promise<Goal> {
  return json(await fetch(`${API_BASE}/goals/${id}`, { method: "DELETE" }));
}

export function formatSessionWhen(session: Session): string {
  const start = new Date(session.start_at);
  const end = new Date(session.end_at);
  return `${start.toLocaleString(undefined, {
    weekday: "short",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  })}–${end.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" })}`;
}
