import {
  CaretLeft,
  CaretRight,
  CheckCircle,
  CircleNotch,
  Plus,
  PencilSimple,
  Trash,
  Stop,
  ArrowUp,
} from "@phosphor-icons/react";
import { FormEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link } from "react-router-dom";
import {
  createConversation,
  decideProposal,
  deleteConversation,
  fetchMessages,
  fetchUsage,
  formatSessionWhen,
  listConversations,
  renameConversation,
  restoreConversation,
  saveDraft,
  sendChat,
  type ActivityStep,
  type ChatResponse,
  type ClarificationQuestion,
  type Conversation,
  type Goal,
  type ProposalSummary,
  type Session,
  type UsageSummary,
} from "../api";

type Msg = { role: "user" | "assistant"; content: string };

const STORAGE_KEY = "buddy.conversationId";

export function ChatPage() {
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [conversationId, setConversationId] = useState<string | null>(
    () => localStorage.getItem(STORAGE_KEY),
  );
  const [messages, setMessages] = useState<Msg[]>([
    {
      role: "assistant",
      content:
        "Tell me a goal in plain language. I'll ask what matters in one go, then we can shape calendar sessions you control.",
    },
  ]);
  const [input, setInput] = useState("");
  const [goal, setGoal] = useState<Goal | null>(null);
  const [proposals, setProposals] = useState<Session[]>([]);
  const [proposalSummary, setProposalSummary] = useState<ProposalSummary | null>(null);
  const [questions, setQuestions] = useState<ClarificationQuestion[]>([]);
  const [answers, setAnswers] = useState<Record<string, string>>({});
  const [activity, setActivity] = useState<ActivityStep[]>([]);
  const [activityOpen, setActivityOpen] = useState(true);
  const [whyOpen, setWhyOpen] = useState(false);
  const [undoBatchId, setUndoBatchId] = useState<string | null>(null);
  const [usage, setUsage] = useState<UsageSummary | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [toast, setToast] = useState<string | null>(null);
  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const composingRef = useRef(false);
  const abortRef = useRef<AbortController | null>(null);
  const endRef = useRef<HTMLDivElement | null>(null);
  const composerRef = useRef<HTMLTextAreaElement | null>(null);

  const refreshSidebar = useCallback(async () => {
    const rows = await listConversations();
    setConversations(rows);
    const u = await fetchUsage().catch(() => null);
    if (u) setUsage(u);
  }, []);

  useEffect(() => {
    refreshSidebar().catch(() => undefined);
  }, [refreshSidebar]);

  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, busy, questions, proposals]);

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
      .catch(() => undefined);
  }, [conversationId]);

  useEffect(() => {
    if (!conversationId || !input) return;
    const t = window.setTimeout(() => {
      saveDraft(conversationId, { composer: input, answers }).catch(() => undefined);
    }, 400);
    return () => window.clearTimeout(t);
  }, [input, answers, conversationId]);

  async function selectConversation(id: string) {
    setConversationId(id);
    localStorage.setItem(STORAGE_KEY, id);
    setProposals([]);
    setProposalSummary(null);
    setQuestions([]);
    setAnswers({});
    setUndoBatchId(null);
    setError(null);
    setActivity([]);
    composerRef.current?.focus();
  }

  async function onNewChat() {
    const created = await createConversation();
    await refreshSidebar();
    await selectConversation(created.id);
    setMessages([
      {
        role: "assistant",
        content:
          "New chat. Tell me what you want to work toward — no Cloud AI was used to open this.",
      },
    ]);
    setInput("");
    setGoal(null);
    composerRef.current?.focus();
  }

  function applyChatResult(res: ChatResponse) {
    setConversationId(res.conversation_id);
    localStorage.setItem(STORAGE_KEY, res.conversation_id);
    setGoal(res.goal);
    setActivity(res.activity || []);
    setActivityOpen(true);
    if (res.proposed_sessions?.length) {
      setProposals(res.proposed_sessions);
      setProposalSummary(res.proposal_summary ?? null);
      setQuestions([]);
    }
    if (res.clarification_questions?.length) {
      setQuestions(res.clarification_questions);
      const next: Record<string, string> = {};
      for (const q of res.clarification_questions) {
        if (q.suggested_answer) next[q.id] = q.suggested_answer;
      }
      setAnswers(next);
    }
    if (res.booked_sessions?.length) {
      setProposals([]);
      setProposalSummary(null);
      setUndoBatchId(res.undo_batch_id ?? null);
    }
    if (res.undo_batch_id) setUndoBatchId(res.undo_batch_id);
    setMessages((m) => [...m, { role: "assistant", content: res.reply }]);
    if (res.ai_available === false) {
      setError("Cloud AI is temporarily unavailable. Today, Calendar and Sparks still work.");
    }
    refreshSidebar().catch(() => undefined);
  }

  async function sendMessage(text: string) {
    if (!text.trim() || busy) return;
    setError(null);
    setToast(null);
    setMessages((m) => [...m, { role: "user", content: text }]);
    setBusy(true);
    setActivity([{ stage: "started", label: "Understanding your goal", detail: null }]);
    const controller = new AbortController();
    abortRef.current = controller;
    try {
      const res = await sendChat(text, conversationId, controller.signal);
      applyChatResult(res);
    } catch (err) {
      if ((err as Error).name === "AbortError") {
        setActivity([{ stage: "cancelled", label: "Stopped", detail: "No database or calendar changes" }]);
        setMessages((m) => [...m, { role: "assistant", content: "Stopped. Your message is saved — nothing else changed." }]);
      } else {
        const message = err instanceof Error ? err.message : "Something went wrong";
        setError(message);
        setActivity([{ stage: "error", label: "Could not reach Cloud AI", detail: "No calendar changes made" }]);
        setMessages((m) => [
          ...m,
          { role: "assistant", content: `I couldn't finish that turn (${message}). Your message is preserved.` },
        ]);
      }
    } finally {
      setBusy(false);
      abortRef.current = null;
      composerRef.current?.focus();
    }
  }

  async function onSubmit(e: FormEvent) {
    e.preventDefault();
    const text = input.trim();
    if (!text || busy) return;
    setInput("");
    await sendMessage(text);
  }

  async function submitQuestionStack() {
    const lines = questions.map((q) => {
      const a = (answers[q.id] || "").trim() || "(skipped)";
      return `${q.label}: ${a}`;
    });
    const payload = `Here are my answers:\n${lines.join("\n")}\nPlease continue with the plan.`;
    setQuestions([]);
    await sendMessage(payload);
  }

  async function onDecide(decision: "approve" | "reject" | "adjust" | "undo") {
    const batchId = proposals[0]?.proposal_batch_id || undoBatchId;
    if (!batchId || busy) return;
    if (decision === "adjust") {
      setMessages((m) => [
        ...m,
        {
          role: "assistant",
          content:
            "What should change? Example: “move Thursday to 19:00” or “drop one climbing night”. Deterministic tweaks won’t use Cloud AI when possible.",
        },
      ]);
      return;
    }
    setBusy(true);
    try {
      const res = await decideProposal(batchId, decision, conversationId);
      if (decision === "approve") {
        setProposals([]);
        setProposalSummary(null);
        setUndoBatchId(res.undo_batch_id || batchId);
        setToast("Booked. You can undo without Cloud AI.");
        setMessages((m) => [
          ...m,
          {
            role: "assistant",
            content: `Booked ${res.booked?.length ?? 0} session(s). They appear in Calendar and Today.`,
          },
        ]);
      } else if (decision === "reject") {
        setProposals([]);
        setProposalSummary(null);
        setMessages((m) => [
          ...m,
          { role: "assistant", content: `Rejected ${res.rejected ?? 0} proposed session(s).` },
        ]);
      } else if (decision === "undo") {
        setUndoBatchId(null);
        setProposals(res.undone || []);
        setToast("Booking undone — sessions are proposals again.");
        setMessages((m) => [
          ...m,
          { role: "assistant", content: "Undid that booking. Nothing used Cloud AI." },
        ]);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Decision failed");
    } finally {
      setBusy(false);
    }
  }

  const usagePct = useMemo(() => {
    if (!usage?.limit) return 0;
    return Math.min(100, Math.round((usage.used / usage.limit) * 100));
  }, [usage]);

  const requiredMissing = questions.some(
    (q) => q.required && !(answers[q.id] || "").trim(),
  );

  return (
    <section className={`chat-workspace${sidebarOpen ? "" : " sidebar-collapsed"}`}>
      <aside className="chat-sidebar" aria-label="Conversations">
        <div className="chat-sidebar-top">
          <button type="button" className="btn primary block" onClick={onNewChat}>
            <Plus size={16} /> New chat
          </button>
          <button
            type="button"
            className="icon-btn"
            title={sidebarOpen ? "Collapse" : "Expand"}
            onClick={() => setSidebarOpen((v) => !v)}
          >
            {sidebarOpen ? <CaretLeft size={16} /> : <CaretRight size={16} />}
          </button>
        </div>
        <div className="chat-list">
          {conversations.map((c) => (
            <div
              key={c.id}
              className={`chat-list-item${c.id === conversationId ? " active" : ""}`}
            >
              {renamingId === c.id ? (
                <form
                  onSubmit={async (e) => {
                    e.preventDefault();
                    await renameConversation(c.id, renameValue);
                    setRenamingId(null);
                    await refreshSidebar();
                  }}
                >
                  <input
                    value={renameValue}
                    onChange={(e) => setRenameValue(e.target.value)}
                    autoFocus
                    aria-label="Rename chat"
                  />
                </form>
              ) : (
                <button type="button" className="chat-title-btn" onClick={() => selectConversation(c.id)}>
                  {c.title}
                </button>
              )}
              <div className="chat-item-actions">
                <button
                  type="button"
                  className="icon-btn"
                  title="Rename"
                  onClick={() => {
                    setRenamingId(c.id);
                    setRenameValue(c.title);
                  }}
                >
                  <PencilSimple size={14} />
                </button>
                <button
                  type="button"
                  className="icon-btn"
                  title="Delete"
                  onClick={async () => {
                    await deleteConversation(c.id);
                    setToast("Moved to Recently Deleted. Undo?");
                    const deletedId = c.id;
                    await refreshSidebar();
                    if (conversationId === deletedId) await onNewChat();
                    window.setTimeout(() => setToast(null), 6000);
                    (window as unknown as { __buddyRestore?: string }).__buddyRestore = deletedId;
                  }}
                >
                  <Trash size={14} />
                </button>
              </div>
            </div>
          ))}
        </div>
        <div className="chat-sidebar-foot">
          {toast?.includes("Undo") && (
            <button
              type="button"
              className="btn ghost block"
              onClick={async () => {
                const id = (window as unknown as { __buddyRestore?: string }).__buddyRestore;
                if (id) {
                  await restoreConversation(id);
                  await refreshSidebar();
                  setToast("Chat restored");
                }
              }}
            >
              Undo delete
            </button>
          )}
          <div className="usage-block" title="Only actual Cloud AI calls count. Retries count too.">
            <div className="usage-label">{usage?.label || "Cloud requests — / — today"}</div>
            <div className="usage-bar" aria-hidden>
              <span style={{ width: `${usagePct}%` }} />
            </div>
            <div className="privacy-label">Local data · Cloud reasoning</div>
          </div>
        </div>
      </aside>

      <div className="chat-main">
        <header className="chat-header">
          <div>
            <h1 className="page-title">Chat</h1>
            <p className="page-sub">Plan with Buddy, then approve what hits the calendar.</p>
          </div>
          {goal && (
            <div className="goal-chip tight">
              Tracking <strong>{goal.title}</strong>
              {goal.baseline ? ` · from ${goal.baseline}` : ""}
              {goal.frequency ? ` · ${goal.frequency}` : ""}
            </div>
          )}
        </header>

        {error && <div className="error-banner">{error}</div>}
        {toast && !toast.includes("Undo") && <div className="toast-banner">{toast}</div>}

        {!!activity.length && (
          <div className="activity-path">
            <button type="button" className="activity-toggle" onClick={() => setActivityOpen((v) => !v)}>
              {busy ? <CircleNotch className="spin" size={14} /> : <CheckCircle size={14} />}
              {activity[activity.length - 1]?.label || "Working"}
            </button>
            {activityOpen && (
              <ol>
                {activity.map((a, i) => (
                  <li key={`${a.stage}-${i}`}>
                    <strong>{a.label}</strong>
                    {a.detail ? <span className="muted"> — {a.detail}</span> : null}
                  </li>
                ))}
              </ol>
            )}
          </div>
        )}

        <div className="messages frosted-scroll">
          {messages.map((m, i) => (
            <div key={`${m.role}-${i}`} className={`bubble ${m.role}`}>
              {m.content}
            </div>
          ))}

          {!!questions.length && (
            <div className="frost-card question-stack">
              <h2>A few details will shape this properly</h2>
              {questions.map((q) => (
                <label key={q.id} className="q-field">
                  <span>
                    {q.label}
                    {q.required ? "" : " (optional)"}
                  </span>
                  {q.help_text && <small className="muted">{q.help_text}</small>}
                  {q.answer_type === "yes_no" || q.answer_type === "single_choice" ? (
                    <select
                      value={answers[q.id] || ""}
                      onChange={(e) => setAnswers((a) => ({ ...a, [q.id]: e.target.value }))}
                    >
                      <option value="">Choose…</option>
                      {(q.options.length ? q.options : ["Yes", "No"]).map((o) => (
                        <option key={o} value={o}>
                          {o}
                        </option>
                      ))}
                    </select>
                  ) : (
                    <input
                      value={answers[q.id] || ""}
                      onChange={(e) => setAnswers((a) => ({ ...a, [q.id]: e.target.value }))}
                      placeholder={q.suggested_answer || ""}
                    />
                  )}
                  {q.reason && <small className="muted">{q.reason}</small>}
                </label>
              ))}
              <div className="actions">
                <button
                  type="button"
                  className="btn primary"
                  disabled={busy || requiredMissing}
                  onClick={() => submitQuestionStack()}
                >
                  Continue · 1 request
                </button>
                <button type="button" className="btn ghost" onClick={() => setQuestions([])}>
                  Cancel
                </button>
              </div>
            </div>
          )}

          {!!proposals.length && proposalSummary && (
            <div className="proposal-duo">
              <div className="frost-card">
                <h2>Goal & approach</h2>
                <p>
                  <strong>{proposalSummary.goal_card?.title || goal?.title}</strong>
                </p>
                <ul className="meta-list">
                  {proposalSummary.goal_card?.outcome && (
                    <li>Outcome: {proposalSummary.goal_card.outcome}</li>
                  )}
                  {proposalSummary.goal_card?.baseline && (
                    <li>Starting from: {proposalSummary.goal_card.baseline}</li>
                  )}
                  {proposalSummary.goal_card?.deadline && (
                    <li>Deadline: {proposalSummary.goal_card.deadline}</li>
                  )}
                  {proposalSummary.pattern && <li>Weekly rhythm: {proposalSummary.pattern}</li>}
                </ul>
                <p className="muted">{proposalSummary.goal_card?.realistic_note}</p>
              </div>
              <div className="frost-card accent">
                <h2>Calendar placement</h2>
                <p className="muted">
                  First week · then through {proposalSummary.through} · {proposalSummary.total}{" "}
                  sessions
                </p>
                <ul className="list">
                  {(proposalSummary.sample || proposals.slice(0, 7)).map((s) => (
                    <li key={s.id}>
                      {formatSessionWhen(s)} · {s.title}
                    </li>
                  ))}
                </ul>
                <button type="button" className="linkish" onClick={() => setWhyOpen((v) => !v)}>
                  {whyOpen ? "Hide" : "Why these times?"}
                </button>
                {whyOpen && (
                  <ul className="meta-list">
                    {(proposalSummary.why_lines || []).map((line) => (
                      <li key={line}>{line}</li>
                    ))}
                    {!proposalSummary.why_lines?.length && (
                      <li>Placed around your availability and protected commitments.</li>
                    )}
                  </ul>
                )}
                <div className="actions">
                  <button type="button" className="btn primary" disabled={busy} onClick={() => onDecide("approve")}>
                    Approve plan
                  </button>
                  <button type="button" className="btn" disabled={busy} onClick={() => onDecide("adjust")}>
                    Adjust
                  </button>
                  <button type="button" className="btn danger" disabled={busy} onClick={() => onDecide("reject")}>
                    Reject
                  </button>
                  <Link className="btn ghost" to="/calendar">
                    View in Calendar
                  </Link>
                </div>
              </div>
            </div>
          )}

          {undoBatchId && !proposals.length && (
            <div className="actions" style={{ marginTop: "0.5rem" }}>
              <button type="button" className="btn" disabled={busy} onClick={() => onDecide("undo")}>
                Undo booking
              </button>
            </div>
          )}

          {busy && <div className="bubble assistant muted">Working…</div>}
          <div ref={endRef} />
        </div>

        <form className="composer frost-composer" onSubmit={onSubmit}>
          <textarea
            ref={composerRef}
            value={input}
            onChange={(e) => setInput(e.target.value)}
            placeholder="What do you want to work toward?"
            rows={2}
            onCompositionStart={() => {
              composingRef.current = true;
            }}
            onCompositionEnd={() => {
              composingRef.current = false;
            }}
            onKeyDown={(e) => {
              if (e.key !== "Enter") return;
              if (composingRef.current || e.nativeEvent.isComposing) return;
              if (e.shiftKey) return;
              e.preventDefault();
              if (!busy && input.trim()) onSubmit(e);
            }}
          />
          {busy ? (
            <button
              type="button"
              className="btn danger"
              onClick={() => abortRef.current?.abort()}
              title="Stop"
            >
              <Stop size={16} /> Stop
            </button>
          ) : (
            <button className="btn primary" type="submit" disabled={!input.trim()}>
              <ArrowUp size={16} /> Send
            </button>
          )}
        </form>
      </div>
    </section>
  );
}
