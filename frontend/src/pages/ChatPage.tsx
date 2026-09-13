import { FormEvent, useEffect, useRef, useState } from "react";
import { formatSessionWhen, sendChat, type Goal, type Session } from "../api";

type Msg = { role: "user" | "assistant"; content: string };

const STORAGE_KEY = "buddy.conversationId";

export function ChatPage() {
  const [messages, setMessages] = useState<Msg[]>([
    {
      role: "assistant",
      content:
        "Tell me a goal in plain language. I'll ask only what I need, then we can turn it into calendar sessions when you're ready.",
    },
  ]);
  const [input, setInput] = useState("");
  const [conversationId, setConversationId] = useState<string | null>(
    () => localStorage.getItem(STORAGE_KEY),
  );
  const [goal, setGoal] = useState<Goal | null>(null);
  const [proposals, setProposals] = useState<Session[]>([]);
  const [busy, setBusy] = useState(false);
  const endRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, busy]);

  async function onSubmit(e: FormEvent) {
    e.preventDefault();
    const text = input.trim();
    if (!text || busy) return;
    setInput("");
    setMessages((m) => [...m, { role: "user", content: text }]);
    setBusy(true);
    try {
      const res = await sendChat(text, conversationId);
      setConversationId(res.conversation_id);
      localStorage.setItem(STORAGE_KEY, res.conversation_id);
      setGoal(res.goal);
      if (res.proposed_sessions?.length) setProposals(res.proposed_sessions);
      if (res.booked_sessions?.length) setProposals([]);
      setMessages((m) => [...m, { role: "assistant", content: res.reply }]);
    } catch (err) {
      const message = err instanceof Error ? err.message : "Something went wrong";
      setMessages((m) => [
        ...m,
        { role: "assistant", content: `I couldn't reach the Buddy service (${message}).` },
      ]);
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="chat-layout">
      <div>
        <h1 className="page-title">Chat</h1>
        <p className="page-sub">Natural conversation that turns unclear goals into a plan you control.</p>
        <div className="messages">
          {messages.map((m, i) => (
            <div key={`${m.role}-${i}`} className={`bubble ${m.role}`}>
              {m.content}
            </div>
          ))}
          {busy && <div className="bubble assistant muted">Thinking…</div>}
          <div ref={endRef} />
        </div>
        {goal && (
          <div className="goal-chip">
            Tracking goal <strong>{goal.title}</strong>
            {goal.baseline ? ` · from ${goal.baseline}` : ""}
            {goal.frequency ? ` · ${goal.frequency}` : ""}
            {goal.status ? ` · ${goal.status}` : ""}
          </div>
        )}
        {!!proposals.length && (
          <div className="panel">
            <h2>Proposed sessions</h2>
            <ul className="list">
              {proposals.map((s) => (
                <li key={s.id}>
                  {formatSessionWhen(s)} · {s.title}
                </li>
              ))}
            </ul>
            <p className="muted">Reply “Approve” or “Reject those” to decide.</p>
          </div>
        )}
      </div>
      <form className="composer" onSubmit={onSubmit}>
        <textarea
          value={input}
          onChange={(e) => setInput(e.target.value)}
          placeholder="What do you want to work toward?"
          rows={2}
        />
        <button type="submit" disabled={busy || !input.trim()}>
          Send
        </button>
      </form>
    </section>
  );
}
