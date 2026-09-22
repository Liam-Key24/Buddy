import { useEffect } from "react";
import { createPortal } from "react-dom";
import { Trash, X } from "@phosphor-icons/react";
import type { Goal } from "../api";
import { Button } from "./ui/Button";
import { FrostFloat } from "./ui/FrostFloat";
import { IconButton } from "./ui/IconButton";

function formatGoalDate(value?: string | null): string {
  if (!value) return "—";
  const trimmed = value.trim();
  if (!trimmed) return "—";
  const monthOnly = /^(\d{4})-(\d{2})$/.exec(trimmed);
  if (monthOnly) {
    const d = new Date(Number(monthOnly[1]), Number(monthOnly[2]) - 1, 1);
    if (!Number.isNaN(d.getTime())) {
      return d.toLocaleDateString(undefined, { month: "short", year: "numeric" });
    }
  }
  const parsed = new Date(trimmed.length <= 10 ? `${trimmed}T12:00:00` : trimmed);
  if (Number.isNaN(parsed.getTime())) return trimmed;
  return parsed.toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-baseline justify-between gap-3">
      <dt className="text-[11px] tracking-wide text-muted uppercase">{label}</dt>
      <dd className="m-0 text-sm text-ink">{value}</dd>
    </div>
  );
}

export function GoalDetailPanel({
  goal,
  busy,
  onClose,
  onRemove,
}: {
  goal: Goal;
  busy?: boolean;
  onClose: () => void;
  onRemove: () => void;
}) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !busy) {
        e.preventDefault();
        onClose();
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [busy, onClose]);

  const summary = [goal.target, goal.baseline && `From ${goal.baseline}`, goal.frequency]
    .filter(Boolean)
    .join(" · ");

  return createPortal(
    <>
      <button
        type="button"
        className="fixed inset-0 z-[80] bg-overlay"
        aria-label="Close goal details"
        onClick={onClose}
      />
      <aside
        className="fixed top-5 right-5 z-[80] w-[min(20rem,calc(100vw-1.5rem))]"
        aria-label="Goal details"
      >
        <FrostFloat className="max-h-[calc(100dvh-2.5rem)] overflow-y-auto p-4">
          <div className="mb-3 flex items-start justify-between gap-2">
            <div>
              <p className="m-0 text-[11px] tracking-wide text-muted uppercase">Goal</p>
              <h2 className="mt-1 mb-0 font-display text-xl font-medium">{goal.title}</h2>
            </div>
            <IconButton label="Close details" onClick={onClose}>
              <X size={16} />
            </IconButton>
          </div>

          {summary && <p className="mt-0 mb-4 text-sm text-ink-soft">{summary}</p>}

          <dl className="m-0 flex flex-col gap-2.5">
            <Stat label="Events" value={String(goal.events_total ?? 0)} />
            <Stat label="Completed" value={String(goal.events_completed ?? 0)} />
            <Stat label="Missed" value={String(goal.events_missed ?? 0)} />
            <Stat label="Start" value={formatGoalDate(goal.started_at)} />
            <Stat label="End" value={formatGoalDate(goal.ended_at)} />
          </dl>

          <div className="mt-4">
            <Button tone="danger" disabled={busy} onClick={onRemove}>
              <Trash size={16} /> Remove
            </Button>
          </div>
        </FrostFloat>
      </aside>
    </>,
    document.body,
  );
}
