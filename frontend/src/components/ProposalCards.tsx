import { Link } from "react-router-dom";
import { formatSessionWhen, type Goal, type ProposalSummary, type Session } from "../api";

type ProposalCardsProps = {
  goal: Goal | null;
  summary: ProposalSummary;
  proposals: Session[];
  whyOpen: boolean;
  busy: boolean;
  onToggleWhy: () => void;
  onDecide: (decision: "approve" | "reject" | "adjust") => void;
};

export function ProposalCards({
  goal,
  summary,
  proposals,
  whyOpen,
  busy,
  onToggleWhy,
  onDecide,
}: ProposalCardsProps) {
  const sample = summary.sample || proposals.slice(0, 5);
  return (
    <div className="frost-card accent proposal-card-simple">
      <h2>{summary.goal_card?.title || goal?.title || "Proposed plan"}</h2>
      <ul className="meta-list">
        {summary.goal_card?.outcome && <li>{summary.goal_card.outcome}</li>}
        {summary.pattern && <li>{summary.pattern}</li>}
        {summary.through && (
          <li>
            Through {summary.through}
            {summary.total ? ` · ${summary.total} sessions` : ""}
          </li>
        )}
      </ul>

      <ul className="list comfort-proposal-list">
        {sample.map((s) => {
          const color = s.category?.color || "#93c5fd";
          return (
            <li
              key={s.id}
              className="comfort-proposal-row"
              style={{ ["--cat-color" as string]: color }}
            >
              <span className="comfort-swatch" style={{ background: color }} />
              <div>
                <strong>{s.title}</strong>
                <div className="muted">
                  {formatSessionWhen(s)}
                  {s.category ? ` · ${s.category.name}` : ""}
                </div>
              </div>
            </li>
          );
        })}
      </ul>

      <button type="button" className="linkish" onClick={onToggleWhy}>
        {whyOpen ? "Hide why" : "Why these times?"}
      </button>
      {whyOpen && (
        <ul className="meta-list">
          {(summary.why_lines || []).map((line) => (
            <li key={line}>{line}</li>
          ))}
          {!summary.why_lines?.length && <li>Placed around your availability.</li>}
        </ul>
      )}

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
        <Link className="btn ghost" to="/calendar">
          Calendar
        </Link>
      </div>
    </div>
  );
}
