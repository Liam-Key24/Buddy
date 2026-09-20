import {
  ArrowRight,
  ArrowUp,
  CheckCircle,
  CircleNotch,
  Plus,
  Stop,
  Target,
} from "@phosphor-icons/react";
import {
  FormEvent,
  KeyboardEvent,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";
import { useChatNav } from "../chatNav";
import { ProposalCards } from "../components/ProposalCards";
import { Button } from "../components/ui/Button";
import { Tag } from "../components/ui/Tag";
import { useToast } from "../components/ui/Toast";
import {
  cancelChat,
  decideProposal,
  fetchMessages,
  fetchOpenProposal,
  notifyCalendarChanged,
  buildProposalSummary,
  saveDraft,
  sendChat,
  type ActivityStep,
  type ChatResponse,
  type ClarificationQuestion,
  type Goal,
  type ProposalSummary,
  type Session,
} from "../api";
import { cn } from "../lib/cn";

type Msg = { role: "user" | "assistant"; content: string };

const EDIT_HINT_KEY = "buddy.editGoalHint";
const COMPOSER_MAX_PX = 180;

const GREETING: Msg = {
  role: "assistant",
  content: "Tell me a goal. I’ll ask a few things, then propose calendar sessions you control.",
};

function activityIcon(stage: string) {
  if (stage === "error") return <Stop size={12} />;
  if (stage === "cancelled") return <Stop size={12} />;
  return <CircleNotch size={12} className="spin" />;
}

export function ChatPage() {
  const { conversationId, setConversationId, newChat, refresh } = useChatNav();
  const { pushToast } = useToast();
  const [messages, setMessages] = useState<Msg[]>([GREETING]);
  const [input, setInput] = useState("");
  const [goal, setGoal] = useState<Goal | null>(null);
  const [proposals, setProposals] = useState<Session[]>([]);
  const [proposalSummary, setProposalSummary] = useState<ProposalSummary | null>(null);
  const [proposalOpen, setProposalOpen] = useState(true);
  const [questions, setQuestions] = useState<ClarificationQuestion[]>([]);
  const [answers, setAnswers] = useState<Record<string, string>>({});
  const [activity, setActivity] = useState<ActivityStep[]>([]);
  const [stepsCollapsed, setStepsCollapsed] = useState(false);
  const [focusStep, setFocusStep] = useState(0);
  const [whyOpen, setWhyOpen] = useState(false);
  const [undoBatchId, setUndoBatchId] = useState<string | null>(null);
  const [modeTag, setModeTag] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const composingRef = useRef(false);
  const abortRef = useRef<AbortController | null>(null);
  const requestIdRef = useRef<string | null>(null);
  const endRef = useRef<HTMLDivElement | null>(null);
  const composerRef = useRef<HTMLTextAreaElement | null>(null);
  const stepsPanelRef = useRef<HTMLDivElement | null>(null);

  const resizeComposer = useCallback(() => {
    const el = composerRef.current;
    if (!el) return;
    el.style.height = "0px";
    el.style.height = `${Math.min(el.scrollHeight, COMPOSER_MAX_PX)}px`;
  }, []);

  useEffect(() => {
    const raw = localStorage.getItem(EDIT_HINT_KEY);
    if (!raw) return;
    try {
      const hint = JSON.parse(raw) as { title?: string; at?: number };
      localStorage.removeItem(EDIT_HINT_KEY);
      if (hint.at && Date.now() - hint.at > 60_000) return;
      const title = hint.title?.trim();
      if (!title) return;
      pushToast(`Editing “${title}”`);
      setInput((prev) => prev || `I'd like to update my goal “${title}”: `);
      composerRef.current?.focus();
    } catch {
      localStorage.removeItem(EDIT_HINT_KEY);
    }
  }, [pushToast]);

  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, busy, proposals]);

  useEffect(() => {
    if (!conversationId) {
      setMessages([GREETING]);
      setGoal(null);
      setProposals([]);
      setProposalSummary(null);
      setQuestions([]);
      setAnswers({});
      setUndoBatchId(null);
      setActivity([]);
      return;
    }
    let cancelled = false;
    fetchMessages(conversationId)
      .then((rows) => {
        if (cancelled) return;
        if (!rows.length) {
          setMessages([GREETING]);
          return;
        }
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
        setGoal(open.goal);
        if (open.proposed_sessions?.length && open.proposal_summary) {
          setProposals(open.proposed_sessions);
          setProposalSummary(open.proposal_summary);
          setProposalOpen(true);
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

  function applyChatResult(res: ChatResponse) {
    setConversationId(res.conversation_id);
    setGoal(res.goal);
    setActivity(res.activity || []);
    if (res.proposed_sessions?.length) {
      setProposals(res.proposed_sessions);
      setProposalSummary(res.proposal_summary ?? null);
      setProposalOpen(true);
      setQuestions([]);
      setModeTag("Plan");
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
    refresh().catch(() => undefined);
  }

  async function sendMessage(text: string) {
    if (!text.trim() || busy) return;
    setError(null);
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
      setProposalOpen(false);
      return;
    }
    setBusy(true);
    try {
      const res = await decideProposal(batchId, decision, conversationId);
      if (decision === "approve") {
        setProposals([]);
        setProposalSummary(null);
        setUndoBatchId(res.undo_batch_id || batchId);
        pushToast("Booked. You can undo without Cloud AI.");
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
        setProposalOpen(true);
        pushToast("Booking undone — sessions are proposals again.");
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

  const requiredMissing = questions.some((q) => q.required && !(answers[q.id] || "").trim());
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
    <section className="relative flex h-full min-h-0 flex-col bg-page">
      <header className="flex items-center gap-2 px-5 pt-3 pb-1">
        {goal && (
          <Tag icon={<Target size={12} weight="duotone" />} tone="mint">
            {goal.title}
            {goal.frequency ? ` · ${goal.frequency}` : ""}
          </Tag>
        )}
      </header>

      {error && (
        <div className="mx-5 rounded-card bg-danger/15 px-3 py-2 text-sm text-danger">{error}</div>
      )}

      <div className="min-h-0 flex-1 overflow-y-auto px-5 py-3">
        <div className="mx-auto flex max-w-2xl flex-col gap-3">
          {messages.map((m, i) => (
            <div
              key={`${m.role}-${i}`}
              className={cn(
                "max-w-[85%] rounded-card border border-hairline px-3.5 py-2.5 text-sm leading-relaxed",
                m.role === "user"
                  ? "ml-auto bg-raised/60"
                  : "bg-page-deep/40 text-ink-soft",
              )}
            >
              {m.content}
            </div>
          ))}

          {undoBatchId && !proposals.length && (
            <div>
              <Button onClick={() => onDecide("undo")} disabled={busy}>
                Undo booking
              </Button>
            </div>
          )}
          <div ref={endRef} />
        </div>
      </div>

      {!!proposals.length && proposalSummary && proposalOpen && (
        <div className="pointer-events-none absolute inset-x-0 bottom-28 z-10 flex justify-center px-4 md:bottom-32 md:justify-end md:pr-8">
          <div className="pointer-events-auto w-full max-w-md">
            <ProposalCards
              goal={goal}
              summary={proposalSummary}
              proposals={proposals}
              whyOpen={whyOpen}
              busy={busy}
              onToggleWhy={() => setWhyOpen((v) => !v)}
              onClose={() => setProposalOpen(false)}
              onDecide={(decision) => onDecide(decision)}
            />
          </div>
        </div>
      )}

      {!!proposals.length && proposalSummary && !proposalOpen && (
        <div className="absolute right-5 bottom-28 z-10">
          <Button tone="primary" onClick={() => setProposalOpen(true)}>
            Review plan
          </Button>
        </div>
      )}

      <div className="mx-auto w-full max-w-2xl px-4 pb-4">
        {showStepsPanel && (
          <div
            ref={stepsPanelRef}
            className="mb-2 rounded-card border border-hairline bg-page-deep/50 px-3 py-2"
            onKeyDown={onStepsKeyDown}
          >
            <div className="flex flex-wrap items-center gap-2">
              <button
                type="button"
                className="inline-flex items-center gap-1.5 text-xs text-muted"
                onClick={() => {
                  if (busy) return;
                  if (questions.length) setStepsCollapsed((v) => !v);
                }}
                aria-expanded={stepsExpanded}
              >
                {busy ? <CircleNotch className="spin" size={14} /> : <CheckCircle size={14} />}
                <span>{busy ? activityLabel || "Generating…" : stepsExpanded ? "Next steps" : activityLabel}</span>
              </button>
              {busy &&
                activity.map((step) => (
                  <Tag key={`${step.stage}-${step.label}`} icon={activityIcon(step.stage)} tone="mint">
                    {step.label}
                  </Tag>
                ))}
            </div>

            {stepsExpanded && (
              <>
                <ul className="mt-2 flex list-none flex-col gap-2 p-0">
                  {questions.map((q, i) => (
                    <li
                      key={q.id}
                      className={cn("rounded-xl px-2 py-1.5", i === focusStep && "bg-raised/40")}
                      onMouseEnter={() => setFocusStep(i)}
                    >
                      <div className="mb-1 flex items-center gap-1 text-xs text-ink-soft">
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
                          className="w-full rounded-lg bg-raised-soft px-2 py-1 text-sm outline-none"
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
                          className="w-full rounded-lg bg-raised-soft px-2 py-1 text-sm outline-none"
                        />
                      )}
                    </li>
                  ))}
                </ul>
                <div className="mt-2 flex gap-2">
                  <Button
                    tone="primary"
                    disabled={busy || requiredMissing}
                    onClick={() => submitQuestionStack()}
                  >
                    Continue
                  </Button>
                  <Button
                    tone="ghost"
                    onClick={() => {
                      setQuestions([]);
                      setStepsCollapsed(true);
                    }}
                  >
                    Cancel
                  </Button>
                </div>
              </>
            )}
          </div>
        )}

        <form
          className="flex items-end gap-2 rounded-float border border-hairline bg-raised-soft/70 px-3 py-2"
          onSubmit={onSubmit}
        >
          <button
            type="button"
            className="mb-1 grid size-8 place-items-center rounded-full text-muted hover:bg-raised hover:text-ink"
            aria-label="New chat"
            onClick={() => void newChat()}
          >
            <Plus size={16} weight="bold" />
          </button>
          <div className="min-w-0 flex-1">
            {modeTag && (
              <div className="mb-1">
                <Tag tone="mint" onDismiss={() => setModeTag(null)}>
                  {modeTag}
                </Tag>
              </div>
            )}
            <label className="sr-only" htmlFor="chat-composer">
              Message
            </label>
            <textarea
              id="chat-composer"
              ref={composerRef}
              value={input}
              onChange={(e) => setInput(e.target.value)}
              placeholder="What do you want to work toward?"
              rows={1}
              className="max-h-[180px] w-full resize-none bg-transparent py-1.5 text-sm outline-none placeholder:text-muted-dim"
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
          </div>
          {busy ? (
            <button
              type="button"
              className="mb-0.5 grid size-9 place-items-center rounded-full bg-danger text-page-deep"
              onClick={() => {
                const rid = requestIdRef.current;
                if (rid) cancelChat(rid).catch(() => undefined);
                abortRef.current?.abort();
              }}
              aria-label="Stop"
            >
              <Stop size={16} weight="bold" />
            </button>
          ) : (
            <button
              className="mb-0.5 grid size-9 place-items-center rounded-full bg-mint text-page-deep disabled:opacity-40"
              type="submit"
              disabled={!input.trim()}
              aria-label="Send"
            >
              <ArrowUp size={16} weight="bold" />
            </button>
          )}
        </form>
      </div>
    </section>
  );
}
