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

export type ChatResponse = {
  conversation_id: string;
  reply: string;
  goal: Goal | null;
  pending_question: string | null;
};

export type TodayResponse = {
  goals: Goal[];
  attention: string[];
  pending_questions: string[];
};

const API_BASE = import.meta.env.VITE_API_BASE ?? "/api";

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
  if (!res.ok) {
    throw new Error(`Chat failed (${res.status})`);
  }
  return res.json();
}

export async function fetchToday(): Promise<TodayResponse> {
  const res = await fetch(`${API_BASE}/today`);
  if (!res.ok) {
    throw new Error(`Today failed (${res.status})`);
  }
  return res.json();
}
