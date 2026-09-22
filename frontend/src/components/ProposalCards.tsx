import { CalendarBlank, X } from "@phosphor-icons/react";
import { Link } from "react-router-dom";
import {
  formatSessionWhen,
  type Goal,
  type MutationPreview,
  type ProposalSummary,
  type Session,
} from "../api";
import { Button } from "./ui/Button";
import { FrostFloat } from "./ui/FrostFloat";

type ProposalCardsProps = {
  goal: Goal | null;
  summary: ProposalSummary;
  proposals: Session[];
  whyOpen: boolean;
  busy: boolean;
  onToggleWhy: () => void;
  onClose?: () => void;
  onDecide: (decision: "approve" | "reject" | "adjust") => void;
};

export function ProposalCards({
  goal,
  summary,
  proposals,
  whyOpen,
  busy,
  onToggleWhy,
  onClose,
  onDecide,
}: ProposalCardsProps) {
  const sample = summary.sample || proposals.slice(0, 5);
  return (
    <FrostFloat className="w-full max-w-md p-4" role="dialog" aria-label="Review plan">
      <div className="mb-2 flex items-start justify-between gap-2">
        <h2 className="m-0 font-display text-lg font-medium">
          {summary.goal_card?.title || goal?.title || "Proposed plan"}
        </h2>
        {onClose && (
          <button
            type="button"
            className="grid size-7 place-items-center rounded-lg text-muted hover:bg-raised-soft hover:text-ink"
            aria-label="Close plan"
            onClick={onClose}
          >
            <X size={14} />
          </button>
        )}
      </div>
      <ul className="m-0 flex list-none flex-col gap-1 p-0 text-sm text-ink-soft">
        {summary.goal_card?.outcome && <li>{summary.goal_card.outcome}</li>}
        {summary.pattern && <li>{summary.pattern}</li>}
        {summary.through && (
          <li>
            Through {summary.through}
            {summary.total ? ` · ${summary.total} sessions` : ""}
          </li>
        )}
      </ul>

      <ul className="mt-3 flex list-none flex-col gap-1.5 p-0">
        {sample.map((s) => {
          const color = s.category?.color || "#c5d9a0";
          return (
            <li key={s.id} className="flex items-start gap-2 text-sm">
              <span className="mt-1 size-2.5 shrink-0 rounded-full" style={{ background: color }} />
              <div>
                <strong className="font-medium">{s.title}</strong>
                <div className="text-xs text-muted">
                  {formatSessionWhen(s)}
                  {s.category ? ` · ${s.category.name}` : ""}
                </div>
              </div>
            </li>
          );
        })}
      </ul>

      <button type="button" className="mt-3 text-xs text-mint" onClick={onToggleWhy}>
        {whyOpen ? "Hide why" : "Why these times?"}
      </button>
      {whyOpen && (
        <ul className="mt-2 flex list-none flex-col gap-1 p-0 text-xs text-muted">
          {(summary.why_lines || []).map((line) => (
            <li key={line}>{line}</li>
          ))}
          {!summary.why_lines?.length && <li>Placed around your availability.</li>}
        </ul>
      )}

      <div className="mt-4 flex flex-wrap gap-2">
        <Button tone="primary" disabled={busy} onClick={() => onDecide("approve")}>
          Approve
        </Button>
        <Button disabled={busy} onClick={() => onDecide("adjust")}>
          Adjust
        </Button>
        <Button tone="danger" disabled={busy} onClick={() => onDecide("reject")}>
          Reject
        </Button>
        <Link
          className="inline-flex items-center gap-1 rounded-pill px-3 py-1.5 text-sm text-ink-soft hover:bg-raised-soft"
          to="/calendar"
        >
          <CalendarBlank size={14} /> Calendar
        </Link>
      </div>
    </FrostFloat>
  );
}

function valueLine(record: Record<string, unknown> | null | undefined) {
  if (!record) return "Remove from calendar";
  const title = typeof record.title === "string" ? record.title : "";
  const start = typeof record.start_at === "string" ? record.start_at : "";
  const end = typeof record.end_at === "string" ? record.end_at : "";
  const status = typeof record.status === "string" ? record.status : "";
  return [title, start && end ? `${start} → ${end}` : start, status].filter(Boolean).join(" · ");
}

type MutationPreviewCardProps = {
  preview: MutationPreview;
  busy: boolean;
  onApprove: () => void;
  onAdjust: () => void;
  onCancel: () => void;
};

export function MutationPreviewCard({
  preview,
  busy,
  onApprove,
  onAdjust,
  onCancel,
}: MutationPreviewCardProps) {
  const records = preview.records || [];
  const kind = (preview.kind || "calendar change").replace(/_/g, " ");
  return (
    <FrostFloat className="w-full max-w-md p-4" role="dialog" aria-label="Review calendar change">
      <h2 className="m-0 font-display text-lg font-medium">Review {kind}</h2>
      {preview.why && <p className="mt-1 mb-0 text-sm text-ink-soft">{preview.why}</p>}
      <ul className="mt-3 flex list-none flex-col gap-2 p-0">
        {records.map((rec) => (
          <li key={rec.id} className="rounded-xl bg-raised-soft/60 px-3 py-2 text-sm">
            <div className="font-medium">{rec.title || rec.id}</div>
            {rec.match_reason && (
              <div className="mt-0.5 text-xs text-muted">{rec.match_reason}</div>
            )}
            <div className="mt-1 text-xs text-ink-soft">
              Now: {valueLine(rec.before || { title: rec.title, start_at: rec.start_at, end_at: rec.end_at, status: rec.status })}
            </div>
            <div className="text-xs text-ink-soft">After: {valueLine(rec.after)}</div>
          </li>
        ))}
      </ul>
      <div className="mt-4 flex flex-wrap gap-2">
        <Button tone="primary" disabled={busy} onClick={onApprove}>
          Approve
        </Button>
        <Button disabled={busy} onClick={onAdjust}>
          Adjust
        </Button>
        <Button tone="danger" disabled={busy} onClick={onCancel}>
          Cancel
        </Button>
      </div>
    </FrostFloat>
  );
}
