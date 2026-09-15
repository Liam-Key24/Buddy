import { useEffect, useMemo, useState } from "react";
import FullCalendar from "@fullcalendar/react";
import dayGridPlugin from "@fullcalendar/daygrid";
import timeGridPlugin from "@fullcalendar/timegrid";
import interactionPlugin from "@fullcalendar/interaction";
import {
  fetchSessions,
  formatSessionWhen,
  markSessionOutcome,
  type Session,
} from "../api";

function eventClass(status: string): string {
  if (status === "proposed") return "evt-proposed";
  if (status === "scheduled") return "evt-scheduled";
  if (status === "completed") return "evt-completed";
  if (status === "missed") return "evt-missed";
  if (status === "fixed") return "evt-fixed";
  return "evt-scheduled";
}

export function CalendarPage() {
  const [sessions, setSessions] = useState<Session[]>([]);
  const [selected, setSelected] = useState<Session | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function reload() {
    setSessions(await fetchSessions());
  }

  useEffect(() => {
    reload().catch((e: Error) => setError(e.message));
  }, []);

  const events = useMemo(
    () =>
      sessions.map((s) => ({
        id: s.id,
        title: s.title,
        start: s.start_at,
        end: s.end_at,
        classNames: [eventClass(s.status)],
        extendedProps: { session: s },
      })),
    [sessions],
  );

  async function onOutcome(outcome: "completed" | "missed") {
    if (!selected || busy) return;
    setBusy(true);
    try {
      await markSessionOutcome(selected.id, outcome);
      await reload();
      setSelected(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Update failed");
    } finally {
      setBusy(false);
    }
  }

  return (
    <section>
      <h1 className="page-title">Calendar</h1>
      <p className="page-sub">
        Fixed commitments, proposed goal sessions, and what you approved, completed, or missed.
      </p>
      {error && <div className="error-banner">{error}</div>}
      <div className="calendar-wrap">
        <FullCalendar
          plugins={[dayGridPlugin, timeGridPlugin, interactionPlugin]}
          initialView="timeGridWeek"
          headerToolbar={{
            left: "prev,next today",
            center: "title",
            right: "dayGridMonth,timeGridWeek,timeGridDay",
          }}
          height="auto"
          events={events}
          nowIndicator
          eventClick={(info) => {
            const session = info.event.extendedProps.session as Session | undefined;
            if (session) setSelected(session);
          }}
        />
      </div>

      {selected && (
        <div className="panel" style={{ marginTop: "0.85rem" }}>
          <h2>Session</h2>
          <p style={{ margin: "0 0 0.35rem" }}>
            <strong>{selected.title}</strong>
          </p>
          <p className="muted" style={{ margin: 0 }}>
            {formatSessionWhen(selected)} · {selected.status}
            {selected.goal_id ? ` · linked goal` : ""}
          </p>
          {selected.status === "scheduled" && (
            <div className="actions">
              <button type="button" className="btn primary" disabled={busy} onClick={() => onOutcome("completed")}>
                Mark completed
              </button>
              <button type="button" className="btn danger" disabled={busy} onClick={() => onOutcome("missed")}>
                Mark missed
              </button>
            </div>
          )}
          <div className="actions">
            <button type="button" className="btn ghost" onClick={() => setSelected(null)}>
              Close
            </button>
          </div>
        </div>
      )}

      <div className="panel">
        <h2>Session list</h2>
        {!sessions.length && (
          <p className="empty-state">No sessions yet — approve a proposal in Chat.</p>
        )}
        <ul className="list">
          {sessions.map((s) => (
            <li key={s.id} className="session-row">
              <div>
                <strong>{s.title}</strong>
                <div className="muted">{formatSessionWhen(s)}</div>
              </div>
              <span className={`badge ${s.status}`}>{s.status}</span>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}
