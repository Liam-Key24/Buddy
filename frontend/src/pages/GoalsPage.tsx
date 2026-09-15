import { useCallback, useEffect, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import {
  ChatCircle,
  PencilSimple,
  Target,
  Trash,
} from "@phosphor-icons/react";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { EmptyState } from "../components/EmptyState";
import { SectionHead } from "../components/SectionHead";
import { deleteGoal, fetchGoals, type Goal } from "../api";

const STORAGE_KEY = "buddy.conversationId";
const EDIT_HINT_KEY = "buddy.editGoalHint";

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
    localStorage.setItem(STORAGE_KEY, goal.conversation_id);
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
      await reload();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Could not remove goal");
    } finally {
      setBusyId(null);
    }
  }

  return (
    <section className="goals-shell">
      <div className="goals-panel goals-hero">
        <div>
          <p className="page-kicker">
            <Target size={16} weight="duotone" />
            Goals
          </p>
          <h1>Your goals</h1>
          <p className="muted">Edit in Chat, or remove here.</p>
        </div>
        <Link className="btn primary" to="/chat">
          <ChatCircle size={16} /> New in Chat
        </Link>
      </div>

      {error && <div className="error-banner">{error}</div>}

      <div className="goals-panel goals-list-panel">
        <SectionHead
          title="Open"
          action={<span className="muted">{goals.length}</span>}
        />

        {!goals.length && (
          <EmptyState>
            No goals yet. Start one in <Link to="/chat">Chat</Link>.
          </EmptyState>
        )}

        <ul className="goals-card-list">
          {goals.map((g) => (
            <li key={g.id} className="goals-card">
              <div className="goals-card-main">
                <div className="goals-card-top">
                  <strong>{g.title}</strong>
                  <span className={`badge ${g.status}`}>{statusLabel(g.status)}</span>
                </div>
                <ul className="goals-meta">
                  {g.target && <li>{g.target}</li>}
                  {g.baseline && <li>From {g.baseline}</li>}
                  {g.frequency && <li>{g.frequency}</li>}
                  {g.deadline && <li>By {g.deadline}</li>}
                </ul>
              </div>
              <div className="goals-card-actions">
                <button
                  type="button"
                  className="btn primary"
                  onClick={() => editInChat(g)}
                  title="Edit in Chat"
                >
                  <PencilSimple size={16} /> Edit
                </button>
                <button
                  type="button"
                  className="btn danger"
                  disabled={busyId === g.id}
                  onClick={() => setPendingRemove(g)}
                >
                  <Trash size={16} /> Remove
                </button>
              </div>
            </li>
          ))}
        </ul>
      </div>

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
