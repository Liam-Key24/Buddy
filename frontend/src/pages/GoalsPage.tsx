import { useCallback, useEffect, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { ChatCircle, PencilSimple, Target, Trash } from "@phosphor-icons/react";
import { GoalDetailPanel } from "../components/GoalDetailPanel";
import { ErrorBanner } from "../components/ui/ErrorBanner";
import { ConfirmDialog } from "../components/ui/ConfirmDialog";
import { EmptyState } from "../components/ui/EmptyState";
import { Button } from "../components/ui/Button";
import { Surface } from "../components/ui/Surface";
import { Tag } from "../components/ui/Tag";
import { useToast } from "../components/ui/Toast";
import { useChatNav } from "../chatNav";
import { deleteGoal, fetchGoals, notifyCalendarChanged, type Goal } from "../api";
import { useConfirmDeletes } from "../useUserSettings";

const EDIT_HINT_KEY = "buddy.editGoalHint";

/** Matches backend `list_managed`: keep completed (`done`); hard-deleted rows never appear. */
const LISTED_STATUSES = new Set([
  "gathering",
  "ready_to_plan",
  "planned",
  "active",
  "paused",
  "done",
]);

function statusTone(status: string): "mint" | "ok" | "warn" | "raised" {
  if (status === "done") return "warn";
  if (status === "active" || status === "planned" || status === "ready_to_plan") return "ok";
  if (status === "gathering") return "warn";
  if (status === "paused") return "raised";
  return "mint";
}

function statusLabel(status: string): string {
  if (status === "gathering") return "Gathering";
  if (status === "ready_to_plan") return "Ready";
  if (status === "planned") return "Planned";
  if (status === "active") return "Active";
  if (status === "paused") return "Paused";
  if (status === "done") return "Done";
  return status;
}

export function GoalsPage() {
  const navigate = useNavigate();
  const { setConversationId } = useChatNav();
  const { pushToast } = useToast();
  const confirmDeletes = useConfirmDeletes();
  const [goals, setGoals] = useState<Goal[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [pendingRemove, setPendingRemove] = useState<Goal | null>(null);
  const [detail, setDetail] = useState<Goal | null>(null);
  const [loaded, setLoaded] = useState(false);

  const reload = useCallback(async () => {
    const rows = await fetchGoals();
    const listed = rows.filter((g) => LISTED_STATUSES.has(g.status));
    setGoals(listed);
    setLoaded(true);
    setDetail((current) => {
      if (!current) return null;
      return listed.find((g) => g.id === current.id) ?? null;
    });
  }, []);

  useEffect(() => {
    reload().catch((e: Error) => setError(e.message));
  }, [reload]);

  function editInChat(goal: Goal) {
    setConversationId(goal.conversation_id);
    localStorage.setItem(
      EDIT_HINT_KEY,
      JSON.stringify({
        goalId: goal.id,
        title: goal.title,
        at: Date.now(),
      }),
    );
    navigate("/chat");
  }

  async function removeGoal(goal: Goal) {
    setBusyId(goal.id);
    setError(null);
    try {
      await deleteGoal(goal.id);
      setPendingRemove(null);
      if (detail?.id === goal.id) setDetail(null);
      setGoals((prev) => prev.filter((g) => g.id !== goal.id));
      notifyCalendarChanged();
      pushToast("Goal removed");
      await reload();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Could not remove goal");
    } finally {
      setBusyId(null);
    }
  }

  function requestRemove(goal: Goal) {
    if (confirmDeletes) setPendingRemove(goal);
    else void removeGoal(goal);
  }

  async function confirmRemove() {
    if (!pendingRemove) return;
    await removeGoal(pendingRemove);
  }

  return (
    <section className="h-full overflow-y-auto p-5">
      <div className="mb-5 flex flex-wrap items-end gap-3">
          <div className="min-w-0">
            <p className="m-0 flex items-center gap-1.5 text-xs tracking-wide text-muted uppercase">
              <Target size={14} weight="duotone" />
              Goals
            </p>
            <h1 className="mt-1 mb-0 font-display text-3xl font-medium">Your goals</h1>
          </div>
          <Link
            className="mb-1 inline-flex items-center gap-1.5 rounded-pill bg-mint px-3 py-1.5 text-sm font-medium text-page-deep no-underline"
            to="/chat"
          >
            <ChatCircle size={16} /> New
          </Link>
        </div>

        {error ? <ErrorBanner className="mb-4">{error}</ErrorBanner> : null}

        {loaded && !goals.length && (
          <EmptyState
            icon={<Target size={28} />}
            action={
              <Link to="/chat" className="text-mint">
                Start in Chat
              </Link>
            }
          >
            No goals yet.
          </EmptyState>
        )}

        <ul className="m-0 grid list-none gap-3 p-0 md:grid-cols-2">
          {goals.map((g) => {
            const done = g.status === "done";
            if (done) {
              return (
                <li key={g.id}>
                  <Surface
                    className="flex h-full min-h-36 flex-col items-center justify-center gap-1 text-center"
                    style={{
                      background: "color-mix(in srgb, var(--color-warn) 18%, var(--color-raised))",
                    }}
                  >
                    <strong className="font-display text-xl font-medium text-warn">{g.title}</strong>
                    <p className="m-0 text-sm text-warn/80">completed</p>
                    <Button className="mt-3" tone="primary" onClick={() => setDetail(g)}>
                      Details
                    </Button>
                  </Surface>
                </li>
              );
            }
            return (
              <li key={g.id}>
                <Surface className="flex h-full flex-col gap-3">
                  <div className="flex items-start justify-between gap-2">
                    <strong>{g.title}</strong>
                    <Tag tone={statusTone(g.status)}>{statusLabel(g.status)}</Tag>
                  </div>
                  <ul className="m-0 flex list-none flex-col gap-0.5 p-0 text-sm text-muted">
                    {g.target && <li>{g.target}</li>}
                    {g.baseline && <li>From {g.baseline}</li>}
                    {g.frequency && <li>{g.frequency}</li>}
                    {g.deadline && <li>By {g.deadline}</li>}
                  </ul>
                  <div className="mt-auto flex gap-2">
                    <Button tone="primary" onClick={() => editInChat(g)} aria-label="Edit in Chat">
                      <PencilSimple size={16} /> Edit
                    </Button>
                    <Button
                      tone="danger"
                      disabled={busyId === g.id}
                      onClick={() => requestRemove(g)}
                    >
                      <Trash size={16} /> Remove
                    </Button>
                  </div>
                </Surface>
              </li>
            );
          })}
        </ul>

      {detail && (
        <GoalDetailPanel
          goal={detail}
          busy={busyId === detail.id}
          onClose={() => setDetail(null)}
          onRemove={() => requestRemove(detail)}
        />
      )}

      <ConfirmDialog
        open={!!pendingRemove}
        title="Remove goal?"
        message={
          pendingRemove ? (
            <>
              Remove <strong>{pendingRemove.title}</strong>? Open proposals for it will be cleared.
            </>
          ) : null
        }
        confirmLabel="Remove"
        cancelLabel="Cancel"
        danger
        busy={!!busyId}
        onConfirm={confirmRemove}
        onCancel={() => !busyId && setPendingRemove(null)}
      />
    </section>
  );
}
