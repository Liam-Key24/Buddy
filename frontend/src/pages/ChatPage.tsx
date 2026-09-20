import {
  ArrowRight,
  ArrowUp,
  CaretLeft,
  CaretRight,
  CheckCircle,
  CircleNotch,
  Plus,
  PencilSimple,
  Trash,
  Stop,
} from "@phosphor-icons/react";
import {
  FormEvent,
  KeyboardEvent,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { ProposalCards } from "../components/ProposalCards";
import {
  cancelChat,
  createConversation,
  decideProposal,
  deleteConversation,
  fetchMessages,
  fetchOpenProposal,
  fetchUsage,
  listConversations,
  renameConversation,
  restoreConversation,
  saveDraft,
  notifyCalendarChanged,
  buildProposalSummary,
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
const EDIT_HINT_KEY = "buddy.editGoalHint";
const COMPOSER_MAX_PX = 180;

export function ChatPage() {
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [conversationId, setConversationId] = useState<string | null>(
    () => localStorage.getItem(STORAGE_KEY),
  );
  const [messages, setMessages] = useState<Msg[]>([
    {
      role: "assistant",
      content: "Tell me a goal. I’ll ask a few things, then propose calendar sessions you control.",
    },
  ]);
  const [input, setInput] = useState("");
  const [goal, setGoal] = useState<Goal | null>(null);
  const [proposals, setProposals] = useState<Session[]>([]);
  const [proposalSummary, setProposalSummary] = useState<ProposalSummary | null>(null);
  const [questions, setQuestions] = useState<ClarificationQuestion[]>([]);
  const [answers, setAnswers] = useState<Record<string, string>>({});
  const [activity, setActivity] = useState<ActivityStep[]>([]);
  const [stepsCollapsed, setStepsCollapsed] = useState(false);
  const [focusStep, setFocusStep] = useState(0);
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
  const requestIdRef = useRef<string | null>(null);
  const endRef = useRef<HTMLDivElement | null>(null);
  const composerRef = useRef<HTMLTextAreaElement | null>(null);
  const stepsPanelRef = useRef<HTMLDivElement | null>(null);

  const refreshSidebar = useCallback(async () => {
    const rows = await listConversations();
    setConversations(rows);
    const u = await fetchUsage().catch(() => null);
    if (u) setUsage(u);
  }, []);

  const resizeComposer = useCallback(() => {
    const el = composerRef.current;
    if (!el) return;
    el.style.height = "0px";
    el.style.height = `${Math.min(el.scrollHeight, COMPOSER_MAX_PX)}px`;
  }, []);

  useEffect(() => {
    refreshSidebar().catch(() => undefined);
  }, [refreshSidebar]);

  useEffect(() => {
    const raw = localStorage.getItem(EDIT_HINT_KEY);
    if (!raw) return;
    try {
      const hint = JSON.parse(raw) as { title?: string; at?: number };
      localStorage.removeItem(EDIT_HINT_KEY);
      if (hint.at && Date.now() - hint.at > 60_000) return;
      const title = hint.title?.trim();
      if (!title) return;
      setToast(`Editing “${title}” — tell Buddy what to change.`);
      setInput((prev) => prev || `I'd like to update my goal “${title}”: `);
      composerRef.current?.focus();
    } catch {
      localStorage.removeItem(EDIT_HINT_KEY);
    }
  }, []);

  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, busy, proposals]);

  useEffect(() => {
    if (!conversationId) return;
    let cancelled = false;
    fetchMessages(conversationId)
      .then((rows) => {
        if (cancelled || !rows.length) return;
        setMessages(
          rows
            .filter((r) => r.role === "user" || r.role === "assistant")
            .map((r) => ({ role: r.role as "user" | "assistant", content: r.content })),
        );
      })
      .catch(() => undefined);
    fetchOpenProposal(conversationId)
      .then((open) => {
        if (cancelled) return;
        if (open.goal) setGoal(open.goal);
        if (open.proposed_sessions?.length && open.proposal_summary) {
          setProposals(open.proposed_sessions);
          setProposalSummary(open.proposal_summary);
        } else {
          setProposals([]);
          setProposalSummary(null);
        }
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [conversationId]);

  useEffect(() => {
    if (!conversationId || !input) return;
    const t = window.setTimeout(() => {
      saveDraft(conversationId, { composer: input, answers }).catch(() => undefined);
    }, 400);
    return () => window.clearTimeout(t);
  }, [input, answers, conversationId]);

  useEffect(() => {
    resizeComposer();
  }, [input, resizeComposer]);

  useEffect(() => {
    if (busy) {
      setStepsCollapsed(true);
      return;
    }
    if (questions.length) {
      setStepsCollapsed(false);
      setFocusStep(0);
    } else {
      setStepsCollapsed(true);
    }
  }, [busy, questions.length]);

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
    setGoal(null);
    setStepsCollapsed(true);
    composerRef.current?.focus();
  }

  async function onNewChat() {
    const created = await createConversation();
    await refreshSidebar();
    await selectConversation(created.id);
    setMessages([
      {
        role: "assistant",
        content: "New chat. What do you want to work toward?",
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
    if (res.deleted_session_ids?.length || res.updated_sessions?.length) {
      setProposals((current) =>
        current.filter((s) => !(res.deleted_session_ids || []).includes(s.id)),
      );
      notifyCalendarChanged();
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
    setStepsCollapsed(true);
    const controller = new AbortController();
    abortRef.current = controller;
    const requestId =
      typeof crypto !== "undefined" && "randomUUID" in crypto
        ? crypto.randomUUID()
        : `req-${Date.now()}`;
    requestIdRef.current = requestId;
    try {
      const res = await sendChat(text, conversationId, controller.signal, requestId);
      applyChatResult(res);
    } catch (err) {
      if ((err as Error).name === "AbortError") {
        setActivity([{ stage: "cancelled", label: "Stopped", detail: "No database or calendar changes" }]);
        setMessages((m) => [
          ...m,
          { role: "assistant", content: "Stopped. Your message is saved — nothing else changed." },
        ]);
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
      requestIdRef.current = null;
      composerRef.current?.focus();
    }
  }

  async function onSubmit(e: FormEvent) {
    e.preventDefault();
    const text = input.trim();
    if (!text || busy) return;
    setInput("");
    requestAnimationFrame(resizeComposer);
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
          content: "What should change? e.g. “move Thursday to 19:00” or “drop one session”.",
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
        notifyCalendarChanged();
      } else if (decision === "reject") {
        setProposals([]);
        setProposalSummary(null);
        setMessages((m) => [
          ...m,
          { role: "assistant", content: `Rejected ${res.rejected ?? 0} proposed session(s).` },
        ]);
        notifyCalendarChanged();
      } else if (decision === "undo") {
        setUndoBatchId(null);
        const undone = res.undone || [];
        setProposals(undone);
        setProposalSummary(buildProposalSummary(goal, undone));
        setToast("Booking undone — sessions are proposals again.");
        setMessages((m) => [
          ...m,
          { role: "assistant", content: "Undid that booking. Nothing used Cloud AI." },
        ]);
        notifyCalendarChanged();
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

  const showStepsPanel = busy || activity.length > 0 || questions.length > 0;
  const stepsExpanded = !busy && questions.length > 0 && !stepsCollapsed;
  const activityLabel = activity[activity.length - 1]?.label || (busy ? "Generating…" : "Ready");

  function focusQuestionControl(index: number) {
    const root = stepsPanelRef.current;
    if (!root) return;
    const controls = root.querySelectorAll<HTMLElement>("[data-step-control]");
    controls[index]?.focus();
  }

  function onStepsKeyDown(e: KeyboardEvent<HTMLDivElement>) {
    if (!stepsExpanded || !questions.length) return;
    const target = e.target as HTMLElement;
    const inField = target.matches("input, textarea, select");

    if (e.key === "Escape") {
      e.preventDefault();
      setStepsCollapsed(true);
      composerRef.current?.focus();
      return;
    }

    if (inField && (e.key === "ArrowUp" || e.key === "ArrowDown")) return;

    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const next =
        e.key === "ArrowDown"
          ? Math.min(questions.length - 1, focusStep + 1)
          : Math.max(0, focusStep - 1);
      setFocusStep(next);
      focusQuestionControl(next);
      return;
    }

    if (e.key === "Enter" && (e.metaKey || e.ctrlKey) && !requiredMissing && !busy) {
      e.preventDefault();
      submitQuestionStack();
    }
  }

  return (
    <section className={`chat-workspace${sidebarOpen ? "" : " sidebar-collapsed"}`}>
      {sidebarOpen && (
        <aside className="chat-panel chat-sidebar" aria-label="Conversations">
          <div className="chat-sidebar-top">
            <button type="button" className="btn primary block" onClick={onNewChat}>
              <Plus size={16} /> New chat
            </button>
            <button
              type="button"
              className="icon-btn"
              title="Collapse"
              onClick={() => setSidebarOpen(false)}
            >
              <CaretLeft size={16} />
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
            <div className="usage-block" title="Only Cloud AI calls count">
              <div className="usage-label">{usage?.label || "Cloud requests — / — today"}</div>
              <div className="usage-bar" aria-hidden>
                <span style={{ width: `${usagePct}%` }} />
              </div>
            </div>
          </div>
        </aside>
      )}

      <div className="chat-panel chat-main">
        <header className="chat-header">
          <div className="chat-title-row">
            {!sidebarOpen && (
              <button
                type="button"
                className="chat-chats-btn"
                onClick={() => setSidebarOpen(true)}
                title="Open chats"
              >
                <CaretRight size={16} />
                <span>Chats</span>
              </button>
            )}
            <h1 className="page-title">Chat</h1>
            {!sidebarOpen && (
              <button type="button" className="chat-chats-btn" onClick={onNewChat} title="New chat">
                <Plus size={16} />
                <span>New</span>
              </button>
            )}
          </div>
          {goal && (
            <div className="goal-chip tight">
              <strong>{goal.title}</strong>
              {goal.frequency ? ` · ${goal.frequency}` : ""}
            </div>
          )}
        </header>

        <div className="chat-banners">
          {error && <div className="error-banner">{error}</div>}
          {toast && !toast.includes("Undo") && <div className="toast-banner">{toast}</div>}
        </div>

        <div className="messages frosted-scroll">
          {messages.map((m, i) => (
            <div key={`${m.role}-${i}`} className={`bubble ${m.role}`}>
              {m.content}
            </div>
          ))}

          {!!proposals.length && proposalSummary && (
            <ProposalCards
              goal={goal}
              summary={proposalSummary}
              proposals={proposals}
              whyOpen={whyOpen}
              busy={busy}
              onToggleWhy={() => setWhyOpen((v) => !v)}
              onDecide={(decision) => onDecide(decision)}
            />
          )}

          {undoBatchId && !proposals.length && (
            <div className="actions" style={{ marginTop: "0.5rem" }}>
              <button type="button" className="btn" disabled={busy} onClick={() => onDecide("undo")}>
                Undo booking
              </button>
            </div>
          )}

          <div ref={endRef} />
        </div>

        <div className="composer-dock">
          {showStepsPanel && (
            <div
              ref={stepsPanelRef}
              className={`thinking-panel${stepsExpanded ? " expanded" : " compact"}`}
              onKeyDown={onStepsKeyDown}
            >
              <div className="thinking-panel-head">
                <button
                  type="button"
                  className="thinking-toggle"
                  onClick={() => {
                    if (busy) return;
                    if (questions.length) setStepsCollapsed((v) => !v);
                  }}
                  aria-expanded={stepsExpanded}
                >
                  {busy ? (
                    <CircleNotch className="spin" size={14} />
                  ) : (
                    <CheckCircle size={14} />
                  )}
                  <span>{busy ? activityLabel || "Generating…" : stepsExpanded ? "Next steps" : activityLabel}</span>
                </button>
              </div>

              {stepsExpanded && (
                <>
                  <ul className="next-steps-list">
                    {questions.map((q, i) => (
                      <li
                        key={q.id}
                        className={`next-step-item${i === focusStep ? " active" : ""}`}
                        onMouseEnter={() => setFocusStep(i)}
                      >
                        <div className="next-step-label">
                          <ArrowRight size={12} />
                          <span>
                            {q.label}
                            {!q.required ? " (optional)" : ""}
                          </span>
                        </div>
                        {q.answer_type === "yes_no" || q.answer_type === "single_choice" ? (
                          <select
                            data-step-control
                            value={answers[q.id] || ""}
                            onChange={(e) => setAnswers((a) => ({ ...a, [q.id]: e.target.value }))}
                            onFocus={() => setFocusStep(i)}
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
                            data-step-control
                            value={answers[q.id] || ""}
                            onChange={(e) => setAnswers((a) => ({ ...a, [q.id]: e.target.value }))}
                            onFocus={() => setFocusStep(i)}
                            placeholder={q.suggested_answer || q.help_text || ""}
                          />
                        )}
                      </li>
                    ))}
                  </ul>
                  <div className="thinking-actions">
                    <button
                      type="button"
                      className="btn primary"
                      disabled={busy || requiredMissing}
                      onClick={() => submitQuestionStack()}
                    >
                      Continue
                    </button>
                    <button
                      type="button"
                      className="btn ghost"
                      onClick={() => {
                        setQuestions([]);
                        setStepsCollapsed(true);
                      }}
                    >
                      Cancel
                    </button>
                  </div>
                </>
              )}
            </div>
          )}

          <form className="composer frost-composer" onSubmit={onSubmit}>
            <textarea
              ref={composerRef}
              value={input}
              onChange={(e) => setInput(e.target.value)}
              placeholder="What do you want to work toward?"
              rows={1}
              onCompositionStart={() => {
                composingRef.current = true;
              }}
              onCompositionEnd={() => {
                composingRef.current = false;
              }}
              onInput={resizeComposer}
              onKeyDown={(e) => {
                if (e.key !== "Enter") return;
                if (composingRef.current || e.nativeEvent.isComposing) return;
                if (e.shiftKey) return;
                e.preventDefault();
                if (!busy && input.trim()) onSubmit(e);
              }}
            />
            <div className="composer-toolbar">
              {busy ? (
                <button
                  type="button"
                  className="composer-send danger"
                  onClick={() => {
                    const rid = requestIdRef.current;
                    if (rid) {
                      cancelChat(rid).catch(() => undefined);
                    }
                    abortRef.current?.abort();
                  }}
                  aria-label="Stop"
                  title="Stop"
                >
                  <Stop size={18} weight="bold" />
                </button>
              ) : (
                <button
                  className="composer-send"
                  type="submit"
                  disabled={!input.trim()}
                  aria-label="Send"
                >
                  <ArrowUp size={18} weight="bold" />
                </button>
              )}
            </div>
          </form>
        </div>
      </div>
    </section>
  );
}
