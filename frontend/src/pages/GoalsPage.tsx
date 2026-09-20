import { useCallback, useEffect, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { ChatCircle, PencilSimple, Target, Trash } from "@phosphor-icons/react";
import { ConfirmDialog } from "../components/ui/ConfirmDialog";
import { EmptyState } from "../components/ui/EmptyState";
import { Button } from "../components/ui/Button";
import { Surface } from "../components/ui/Surface";
import { Tag } from "../components/ui/Tag";
import { useToast } from "../components/ui/Toast";
import { useChatNav } from "../chatNav";
import { deleteGoal, fetchGoals, type Goal } from "../api";

const EDIT_HINT_KEY = "buddy.editGoalHint";

function statusTone(status: string): "mint" | "ok" | "warn" | "raised" {
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
  return status;
}

export function GoalsPage() {
  const navigate = useNavigate();
  const { setConversationId } = useChatNav();
  const { pushToast } = useToast();
  const [goals, setGoals] = useState<Goal[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [pendingRemove, setPendingRemove] = useState<Goal | null>(null);

  const reload = useCallback(async () => {
    setGoals(await fetchGoals());
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

  async function confirmRemove() {
    if (!pendingRemove) return;
    const goal = pendingRemove;
    setBusyId(goal.id);
    setError(null);
    try {
      await deleteGoal(goal.id);
      setPendingRemove(null);
      pushToast("Goal removed");
      await reload();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Could not remove goal");
    } finally {
      setBusyId(null);
    }
  }

  return (
    <section className="h-full overflow-y-auto p-5">
      <div className="mb-5 flex flex-wrap items-end justify-between gap-3">
        <div>
          <p className="m-0 flex items-center gap-1.5 text-xs tracking-wide text-muted uppercase">
            <Target size={14} weight="duotone" />
            Goals
          </p>
          <h1 className="mt-1 mb-0 font-display text-3xl font-medium">Your goals</h1>
        </div>
        <Link
          className="inline-flex items-center gap-1.5 rounded-pill bg-mint px-3 py-1.5 text-sm font-medium text-page-deep no-underline"
          to="/chat"
        >
          <ChatCircle size={16} /> New
        </Link>
      </div>

      {error && (
        <div className="mb-4 rounded-card bg-danger/15 px-3 py-2 text-sm text-danger">{error}</div>
      )}

      {!goals.length && (
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
        {goals.map((g) => (
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
                  onClick={() => setPendingRemove(g)}
                >
                  <Trash size={16} /> Remove
                </Button>
              </div>
            </Surface>
          </li>
        ))}
      </ul>

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
