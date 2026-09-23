import {
  ArrowRight,
  ArrowUp,
  CheckCircle,
  CircleNotch,
  PencilSimple,
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
import {
  chatResultBelongsToView,
  draftPayload,
  shouldPersistDraft,
  type ChatViewBinding,
} from "../chatSession";
import { useChatNav } from "../chatNav";
import { ProposalCards, MutationPreviewCard } from "../components/ProposalCards";
import { Button } from "../components/ui/Button";
import { useGoalComplete } from "../components/ui/GoalCompleteOverlay";
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
  type MutationPreview,
  type ProposalGroup,
  type ProposalSummary,
  type Session,
} from "../api";
import { cn } from "../lib/cn";

type Msg = { id?: string; role: "user" | "assistant"; content: string };

const EDIT_HINT_KEY = "buddy.editGoalHint";
const CONTINUE_HINT_KEY = "buddy.continueHint";
const COMPOSER_MAX_PX = 180;

const GREETING: Msg = {
  role: "assistant",
  content: "Tell me a goal. I’ll ask a few things, then propose calendar sessions you control.",
};

export function ChatPage() {
  const { conversationId, conversations, setConversationId, renameChat, refresh } = useChatNav();
  const { pushToast } = useToast();
  const { celebrateGoalComplete } = useGoalComplete();
  const [messages, setMessages] = useState<Msg[]>([GREETING]);
  const [input, setInput] = useState("");
  const [goal, setGoal] = useState<Goal | null>(null);
  const [proposals, setProposals] = useState<Session[]>([]);
  const [proposalSummary, setProposalSummary] = useState<ProposalSummary | null>(null);
  const [proposalGroups, setProposalGroups] = useState<ProposalGroup[]>([]);
  const [mutationPreview, setMutationPreview] = useState<MutationPreview | null>(null);
  const [proposalOpen, setProposalOpen] = useState(true);
  const [questions, setQuestions] = useState<ClarificationQuestion[]>([]);
  const [answers, setAnswers] = useState<Record<string, string>>({});
  const [activity, setActivity] = useState<ActivityStep[]>([]);
  const [stepsCollapsed, setStepsCollapsed] = useState(false);
  const [focusStep, setFocusStep] = useState(0);
  const [whyOpen, setWhyOpen] = useState<Record<string, boolean>>({});
  const [undoBatchId, setUndoBatchId] = useState<string | null>(null);
  const [modeTag, setModeTag] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editingTitle, setEditingTitle] = useState(false);
  const [titleDraft, setTitleDraft] = useState("");
  const composingRef = useRef(false);
  const abortRef = useRef<AbortController | null>(null);
  const requestIdRef = useRef<string | null>(null);
  const conversationIdRef = useRef<string | null>(conversationId);
  const viewIdRef = useRef(0);
  const lastViewConversationRef = useRef<string | null>(conversationId);
  const lateReadyRef = useRef<Set<string>>(new Set());
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
    conversationIdRef.current = conversationId;
    if (lastViewConversationRef.current !== conversationId) {
      viewIdRef.current += 1;
      lastViewConversationRef.current = conversationId;
    }
  }, [conversationId]);

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
    const raw = localStorage.getItem(CONTINUE_HINT_KEY);
    if (!raw) return;
    try {
      const hint = JSON.parse(raw) as {
        title?: string;
        detail?: string | null;
        kind?: string;
        at?: number;
      };
      localStorage.removeItem(CONTINUE_HINT_KEY);
      if (hint.at && Date.now() - hint.at > 60_000) return;
      const detail = hint.detail?.trim();
      const title = hint.title?.trim();
      if (title) pushToast(title);
      if (hint.kind === "gathering" && detail) {
        setInput((prev) => prev || `${detail} `);
      } else if (hint.kind === "approve") {
        setInput((prev) => prev || "Approve");
      }
      composerRef.current?.focus();
    } catch {
      localStorage.removeItem(CONTINUE_HINT_KEY);
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
      setProposalGroups([]);
      setMutationPreview(null);
      setQuestions([]);
      setAnswers({});
      setUndoBatchId(null);
      setActivity([]);
      setEditingId(null);
      setError(null);
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
            .map((r) => ({
              id: r.id,
              role: r.role as "user" | "assistant",
              content: r.content,
            })),
        );
      })
      .catch(() => undefined);
    fetchOpenProposal(conversationId)
      .then((open) => {
        if (cancelled) return;
        setGoal(open.goal);
        const groups = open.proposal_groups?.filter((g) => g.sessions?.length) || [];
        if (groups.length) {
          setProposalGroups(groups);
          setProposals(groups[0].sessions);
          setProposalSummary(groups[0].summary);
          setProposalOpen(true);
        } else if (open.proposed_sessions?.length && open.proposal_summary) {
          setProposals(open.proposed_sessions);
          setProposalSummary(open.proposal_summary);
          setProposalGroups([]);
          setProposalOpen(true);
        } else {
          setProposals([]);
          setProposalSummary(null);
          setProposalGroups([]);
        }
        const preview = open.mutation_preview;
        setMutationPreview(preview?.records?.length ? preview : null);
        if (open.clarification_questions?.length) {
          setQuestions(open.clarification_questions);
        } else {
          setQuestions([]);
        }
        const entered = open.entered_answers || open.clarification_answers || {};
        setAnswers(entered);
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [conversationId]);

  useEffect(() => {
    if (!shouldPersistDraft(conversationId, input, answers)) return;
    const t = window.setTimeout(() => {
      saveDraft(conversationId, draftPayload(input, answers)).catch(() => undefined);
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

  function applyChatResult(res: ChatResponse, origin: ChatViewBinding): boolean {
    if (
      !chatResultBelongsToView(
        origin,
        conversationIdRef.current,
        viewIdRef.current,
        res.conversation_id,
      )
    ) {
      lateReadyRef.current.add(res.conversation_id);
      refresh().catch(() => undefined);
      return false;
    }
    if (!conversationIdRef.current) {
      setConversationId(res.conversation_id);
    }
    const prevStatus = goal?.status;
    setGoal(res.goal);
    if (res.goal?.status === "done" && prevStatus !== "done") {
      celebrateGoalComplete(res.goal.title, `${res.conversation_id}:${res.goal.id}`);
    }
    setActivity(res.activity || []);
    const groups = res.proposal_groups?.filter((g) => g.sessions?.length) || [];
    if (groups.length) {
      setProposalGroups(groups);
      setProposals(groups[0].sessions);
      setProposalSummary(groups[0].summary);
      setProposalOpen(true);
      setQuestions([]);
      setModeTag(groups.length > 1 ? `${groups.length} plans` : "Plan");
    } else if (res.proposed_sessions?.length) {
      setProposals(res.proposed_sessions);
      setProposalSummary(res.proposal_summary ?? null);
      setProposalGroups([]);
      setProposalOpen(true);
      setQuestions([]);
      setModeTag("Plan");
    }
    if (res.mutation_preview?.records?.length) {
      setMutationPreview(res.mutation_preview);
    } else {
      setMutationPreview(null);
    }
    if (res.clarification_questions?.length) {
      setQuestions(res.clarification_questions);
    }
    if (res.booked_sessions?.length) {
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
    return true;
  }

  async function reloadMessages(cid: string) {
    const rows = await fetchMessages(cid);
    if (!rows.length) {
      setMessages([GREETING]);
      return;
    }
    setMessages(
      rows
        .filter((r) => r.role === "user" || r.role === "assistant")
        .map((r) => ({
          id: r.id,
          role: r.role as "user" | "assistant",
          content: r.content,
        })),
    );
  }

  function startEdit(message: Msg) {
    if (!message.id || message.role !== "user" || busy) return;
    setEditingId(message.id);
    setInput(message.content);
    setError(null);
    requestAnimationFrame(() => {
      resizeComposer();
      composerRef.current?.focus();
    });
  }

  function cancelEdit() {
    setEditingId(null);
    setInput("");
    requestAnimationFrame(resizeComposer);
  }

  async function sendMessage(
    text: string,
    clarificationAnswers?: Array<{ question_id: string; answer: string }>,
    revisionOf?: string | null,
  ) {
    if (!text.trim() || busy) return;
    setError(null);
    if (!revisionOf) {
      setMessages((m) => [...m, { role: "user", content: text }]);
    }
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
    const origin: ChatViewBinding = {
      originConversationId: conversationId,
      originViewId: viewIdRef.current,
    };
    try {
      const res = await sendChat(text, conversationId, controller.signal, requestId, {
        clarification_answers: clarificationAnswers,
        revision_of: revisionOf || null,
      });
      const applied = applyChatResult(res, origin);
      if (!applied) {
        return;
      }
      const cid = res.conversation_id || conversationId;
      if (cid) {
        try {
          await reloadMessages(cid);
        } catch {
          /* keep optimistic transcript */
        }
      }
      setEditingId(null);
    } catch (err) {
      if ((err as Error).name === "AbortError") {
        let committed = false;
        const rid = requestIdRef.current;
        if (rid) {
          try {
            const info = await cancelChat(rid);
            committed = Boolean(info.committed);
          } catch {
            /* ignore */
          }
        }
        if (committed) {
          setActivity([{ stage: "completed", label: "Finished before Stop", detail: "Changes may have been applied" }]);
          setMessages((m) => [
            ...m,
            { role: "assistant", content: "That turn was already saving — some calendar changes may have been applied." },
          ]);
        } else {
          setActivity([{ stage: "cancelled", label: "Stopped", detail: "No calendar changes" }]);
          setMessages((m) => [
            ...m,
            { role: "assistant", content: "Stopped. Your message is saved — nothing else changed." },
          ]);
        }
      } else {
        const message = err instanceof Error ? err.message : "Something went wrong";
        setError(message);
        if (revisionOf) {
          pushToast(message);
          setInput(text);
          setActivity([{ stage: "error", label: "Edit blocked", detail: "Calendar records were kept" }]);
        } else {
          setActivity([{ stage: "error", label: "Could not reach Cloud AI", detail: "No calendar changes made" }]);
          setMessages((m) => [
            ...m,
            { role: "assistant", content: `I couldn't finish that turn (${message}). Your message is preserved.` },
          ]);
        }
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
    await sendMessage(text, undefined, editingId);
  }

  async function submitQuestionStack() {
    const payload = questions
      .map((q) => ({ question_id: q.id, answer: (answers[q.id] || "").trim() }))
      .filter((row) => row.answer);
    setQuestions([]);
    await sendMessage("Answered the questions.", payload);
  }

  async function onDecide(
    decision: "approve" | "reject" | "adjust" | "undo",
    batchId?: string | null,
  ) {
    const resolvedBatch = batchId || proposalGroups[0]?.proposal_batch_id || proposals[0]?.proposal_batch_id || undoBatchId;
    if (decision === "adjust") {
      setMessages((m) => [
        ...m,
        {
          role: "assistant",
          content: "What should change? Describe the adjustment — I won't apply the current preview until you approve a new one.",
        },
      ]);
      if (!batchId) setProposalOpen(false);
      return;
    }
    if (!resolvedBatch || busy) return;
    setBusy(true);
    try {
      const res = await decideProposal(resolvedBatch, decision, conversationId);
      if (decision === "approve") {
        const remaining = res.proposal_groups?.filter((g) => g.sessions?.length) || [];
        setProposalGroups(remaining);
        setProposals(remaining[0]?.sessions || []);
        setProposalSummary(remaining[0]?.summary ?? null);
        setUndoBatchId(res.undo_batch_id || resolvedBatch);
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
        const remaining = res.proposal_groups?.filter((g) => g.sessions?.length) || [];
        setProposalGroups(remaining);
        setProposals(remaining[0]?.sessions || []);
        setProposalSummary(remaining[0]?.summary ?? null);
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
        setProposalGroups([]);
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

  const currentChat = conversations.find((c) => c.id === conversationId) || null;
  const chatTitle = currentChat?.title || "New chat";

  async function commitTitle() {
    setEditingTitle(false);
    const next = titleDraft.trim();
    if (!conversationId || !next || next === chatTitle) return;
    await renameChat(conversationId, next);
  }

  return (
    <section className="relative flex h-full min-h-0 flex-col bg-page">
      <header className="flex flex-wrap items-center gap-2 px-5 pt-3 pb-1">
        {editingTitle && conversationId ? (
          <input
            autoFocus
            value={titleDraft}
            aria-label="Chat title"
            className="min-w-0 flex-1 bg-transparent font-display text-xl font-medium outline-none"
            onChange={(e) => setTitleDraft(e.target.value)}
            onBlur={() => void commitTitle()}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                void commitTitle();
              }
              if (e.key === "Escape") {
                setEditingTitle(false);
                setTitleDraft(chatTitle);
              }
            }}
          />
        ) : (
          <button
            type="button"
            className="min-w-0 truncate text-left font-display text-xl font-medium text-ink"
            onClick={() => {
              if (!conversationId) return;
              setTitleDraft(chatTitle);
              setEditingTitle(true);
            }}
            title={conversationId ? "Rename chat" : undefined}
          >
            {chatTitle}
          </button>
        )}
        {goal && (
          <Tag icon={<Target size={12} weight="duotone" />} tone="mint">
            {goal.title}
            {goal.frequency ? ` · ${goal.frequency}` : ""}
          </Tag>
        )}
        {!goal && proposalGroups.length > 1 && (
          <Tag icon={<Target size={12} weight="duotone" />} tone="mint">
            {proposalGroups.length} goals
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
              key={m.id || `${m.role}-${i}`}
              className={cn(
                "group relative max-w-[85%] rounded-card border border-hairline px-3.5 py-2.5 text-sm leading-relaxed",
                m.role === "user"
                  ? "ml-auto bg-raised/60"
                  : "bg-page-deep/40 text-ink-soft",
                editingId && m.id === editingId && "border-mint/50",
              )}
            >
              {m.content}
              {m.role === "user" && m.id && !busy && (
                <button
                  type="button"
                  className="mt-1 inline-flex items-center gap-1 text-[11px] text-muted hover:text-ink"
                  onClick={() => startEdit(m)}
                >
                  <PencilSimple size={12} />
                  Edit
                </button>
              )}
            </div>
          ))}

          {undoBatchId && !proposals.length && !proposalGroups.length && (
            <div>
              <Button onClick={() => onDecide("undo")} disabled={busy}>
                Undo booking
              </Button>
            </div>
          )}
          <div ref={endRef} />
        </div>
      </div>

      {(!!proposalGroups.length || (!!proposals.length && proposalSummary) || !!mutationPreview) && proposalOpen && (
        <div className="pointer-events-none absolute inset-x-0 bottom-28 z-10 flex justify-center px-4 md:bottom-32 md:justify-end md:pr-8">
          <div className="pointer-events-auto flex w-full max-w-md max-h-[55vh] flex-col gap-3 overflow-y-auto">
            {mutationPreview && (
              <MutationPreviewCard
                preview={mutationPreview}
                busy={busy}
                onApprove={() => sendMessage("Approve")}
                onAdjust={() => onDecide("adjust")}
                onCancel={() => sendMessage("Cancel")}
              />
            )}
            {(proposalGroups.length
              ? proposalGroups
              : proposalSummary
                ? [{ goal, proposal_batch_id: proposals[0]?.proposal_batch_id || "", summary: proposalSummary, sessions: proposals }]
                : []
            ).map((group) => (
              group.summary ? (
                <ProposalCards
                  key={group.proposal_batch_id}
                  goal={group.goal}
                  summary={group.summary}
                  proposals={group.sessions}
                  whyOpen={Boolean(whyOpen[group.proposal_batch_id])}
                  busy={busy}
                  onToggleWhy={() =>
                    setWhyOpen((current) => ({
                      ...current,
                      [group.proposal_batch_id]: !current[group.proposal_batch_id],
                    }))
                  }
                  onClose={() => setProposalOpen(false)}
                  onDecide={(decision) => onDecide(decision, group.proposal_batch_id)}
                />
              ) : null
            ))}
          </div>
        </div>
      )}

      {(!!proposalGroups.length || (!!proposals.length && proposalSummary) || !!mutationPreview) && !proposalOpen && (
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
              {busy ? (
                <Tag
                  icon={<CircleNotch className="spin" size={12} />}
                  tone="mint"
                >
                  {activityLabel || "Generating…"}
                </Tag>
              ) : (
                <button
                  type="button"
                  className="inline-flex items-center gap-1.5 text-xs text-muted"
                  onClick={() => {
                    if (questions.length) setStepsCollapsed((v) => !v);
                  }}
                  aria-expanded={stepsExpanded}
                >
                  <CheckCircle size={14} />
                  <span>{stepsExpanded ? "Next steps" : activityLabel}</span>
                </button>
              )}
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
                          placeholder={q.help_text || "Your answer"}
                          className="w-full rounded-lg bg-raised-soft px-2 py-1 text-sm outline-none"
                        />
                      )}
                      {q.suggested_answer && (
                        <button
                          type="button"
                          className="mt-1 text-left text-xs text-mint"
                          onClick={() =>
                            setAnswers((a) => ({ ...a, [q.id]: a[q.id] || q.suggested_answer || "" }))
                          }
                        >
                          Suggestion: {q.suggested_answer}
                        </button>
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
          <div className="min-w-0 flex-1">
            {editingId && (
              <div className="mb-1 flex items-center justify-between gap-2 text-[11px] text-ink-soft">
                <span>Editing this message undoes later goal and calendar changes.</span>
                <button type="button" className="text-muted hover:text-ink" onClick={cancelEdit}>
                  Cancel
                </button>
              </div>
            )}
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
              placeholder={editingId ? "Edit your message" : "What do you want to work toward?"}
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
                if (e.key === "Escape" && editingId) {
                  e.preventDefault();
                  cancelEdit();
                  return;
                }
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
