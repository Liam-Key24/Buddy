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
};

export type Spark = {
  id: string;
  content: string;
  status: string;
  promoted_goal_id?: string | null;
  created_at: string;
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
};

export type TodayResponse = {
  goals: Goal[];
  attention: string[];
  pending_questions: string[];
  todays_sessions: Session[];
  progress: Array<{
    goal_id: string;
    title: string;
    scheduled: number;
    completed: number;
    missed: number;
    proposed: number;
  }>;
};

const API_BASE = import.meta.env.VITE_API_BASE ?? "/api";

async function json<T>(res: Response): Promise<T> {
  if (!res.ok) throw new Error(`Request failed (${res.status})`);
  return res.json();
}

export async function sendChat(
  message: string,
  conversationId?: string | null,
): Promise<ChatResponse> {
  const res = await fetch(`${API_BASE}/chat`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      message,
      conversation_id: conversationId || null,
    }),
  });
  return json(res);
}

export async function fetchToday(): Promise<TodayResponse> {
  return json(await fetch(`${API_BASE}/today`));
}

export async function fetchSessions(): Promise<Session[]> {
  return json(await fetch(`${API_BASE}/calendar/sessions`));
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

export async function promoteSpark(id: string): Promise<unknown> {
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
