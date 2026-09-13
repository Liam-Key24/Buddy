import { FormEvent, useEffect, useRef, useState } from "react";
import {
  decideProposal,
  fetchMessages,
  formatSessionWhen,
  sendChat,
  type Goal,
  type Session,
} from "../api";

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
  const [proposalSummary, setProposalSummary] = useState<{
    pattern?: string | null;
    total?: number;
    through?: string | null;
    sample?: Session[];
  } | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const endRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, busy]);

  useEffect(() => {
    if (!conversationId) return;
    fetchMessages(conversationId)
      .then((rows) => {
        if (!rows.length) return;
        setMessages(
          rows
            .filter((r) => r.role === "user" || r.role === "assistant")
            .map((r) => ({ role: r.role as "user" | "assistant", content: r.content })),
        );
      })
      .catch(() => {
        /* keep welcome message if history unavailable */
      });
  }, [conversationId]);

  async function onSubmit(e: FormEvent) {
    e.preventDefault();
    const text = input.trim();
    if (!text || busy) return;
    setInput("");
    setError(null);
    setMessages((m) => [...m, { role: "user", content: text }]);
    setBusy(true);
    try {
      const res = await sendChat(text, conversationId);
      setConversationId(res.conversation_id);
      localStorage.setItem(STORAGE_KEY, res.conversation_id);
      setGoal(res.goal);
      if (res.proposed_sessions?.length) {
        setProposals(res.proposed_sessions);
        setProposalSummary(res.proposal_summary ?? null);
      }
      if (res.booked_sessions?.length) {
        setProposals([]);
        setProposalSummary(null);
      }      setMessages((m) => [...m, { role: "assistant", content: res.reply }]);
      if (res.ai_available === false) {
        setError("Cloud AI is temporarily unavailable. Today, Calendar and Sparks still work.");
      }
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

  async function onDecide(decision: "approve" | "reject" | "adjust") {
    const batchId = proposals[0]?.proposal_batch_id;
    if (!batchId || busy) return;
    setBusy(true);
    setError(null);
    try {
      if (decision === "adjust") {
        setMessages((m) => [
          ...m,
          {
            role: "assistant",
            content:
              "Tell me what to change (fewer nights, different hours, later start) and I'll propose again. Nothing is booked until you approve.",
          },
        ]);
        return;
      }
      const res = await decideProposal(batchId, decision);
      if (decision === "approve") {
        const n = res.booked?.length ?? 0;
        setProposals([]);
        setProposalSummary(null);
        setMessages((m) => [
          ...m,
          {
            role: "assistant",
            content: `Booked ${n} session(s). They'll show in Calendar and Today. Mark them completed or missed when you know.`,
          },
        ]);
      } else {
        setProposals([]);
        setProposalSummary(null);
        setMessages((m) => [
          ...m,
          {
            role: "assistant",
            content: `Rejected ${res.rejected ?? 0} proposed session(s). Say what to change and I'll propose again.`,
          },
        ]);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Decision failed");
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="chat-layout">
      <div>
        <h1 className="page-title">Chat</h1>
        <p className="page-sub">
          Natural conversation that turns unclear goals into a plan you control.
        </p>
        {error && <div className="error-banner">{error}</div>}
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
            Tracking <strong>{goal.title}</strong>
            {goal.baseline ? ` · from ${goal.baseline}` : ""}
            {goal.frequency ? ` · ${goal.frequency}` : ""}
          </div>
        )}
        {!!proposals.length && (
          <div className="proposal-card">
            <h2 style={{ margin: "0 0 0.55rem", fontSize: "0.95rem" }}>Proposed plan</h2>
            {proposalSummary?.pattern && (
              <p style={{ margin: "0 0 0.65rem" }}>
                <strong>Weekly rhythm:</strong> {proposalSummary.pattern}
              </p>
            )}
            <p className="muted" style={{ margin: "0 0 0.55rem" }}>
              First week preview
              {proposalSummary?.through
                ? ` · then repeats through ${proposalSummary.through}`
                : ""}
              {proposalSummary?.total ? ` · ${proposalSummary.total} sessions total` : ""}
            </p>
            <ul className="list">
              {(proposalSummary?.sample?.length
                ? proposalSummary.sample
                : proposals.slice(0, 7)
              ).map((s) => (
                <li key={s.id}>
                  {formatSessionWhen(s)} · {s.title}
                </li>
              ))}
            </ul>
            <div className="actions">
              <button type="button" className="btn primary" disabled={busy} onClick={() => onDecide("approve")}>
                Approve
              </button>
              <button type="button" className="btn" disabled={busy} onClick={() => onDecide("adjust")}>
                Adjust
              </button>
              <button type="button" className="btn danger" disabled={busy} onClick={() => onDecide("reject")}>
                Reject
              </button>
            </div>
          </div>
        )}
      </div>
      <form className="composer" onSubmit={onSubmit}>
        <textarea
          value={input}
          onChange={(e) => setInput(e.target.value)}
          placeholder="What do you want to work toward?"
          rows={2}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              onSubmit(e as unknown as FormEvent);
            }
          }}
        />
        <button className="btn primary" type="submit" disabled={busy || !input.trim()}>
          Send
        </button>
      </form>
    </section>
  );
}
