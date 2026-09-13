import { useEffect, useMemo, useState } from "react";
import FullCalendar from "@fullcalendar/react";
import dayGridPlugin from "@fullcalendar/daygrid";
import timeGridPlugin from "@fullcalendar/timegrid";
import interactionPlugin from "@fullcalendar/interaction";
import { fetchSessions, formatSessionWhen, type Session } from "../api";

const STATUS_COLOR: Record<string, string> = {
  proposed: "#8eb5c8",
  scheduled: "#c8e08a",
  completed: "#9ecb8a",
  missed: "#e8a598",
  rejected: "#777",
};

export function CalendarPage() {
  const [sessions, setSessions] = useState<Session[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    fetchSessions()
      .then(setSessions)
      .catch((e: Error) => setError(e.message));
  }, []);

  const events = useMemo(
    () =>
      sessions.map((s) => ({
        id: s.id,
        title: `${s.title} (${s.status})`,
        start: s.start_at,
        end: s.end_at,
        backgroundColor: STATUS_COLOR[s.status] ?? "#c8e08a",
        borderColor: "transparent",
        textColor: "#1a2420",
      })),
    [sessions],
  );

  return (
    <section>
      <h1 className="page-title">Calendar</h1>
      <p className="page-sub">
        Buddy’s own calendar for fixed, flexible, proposed, completed, and missed sessions.
      </p>
      {error && <p className="muted">{error}</p>}
      <div className="panel calendar-panel">
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
        />
      </div>
      <div className="panel">
        <h2>Session list</h2>
        {!sessions.length && <p className="muted">No sessions yet — approve a proposal in Chat.</p>}
        <ul className="list">
          {sessions.map((s) => (
            <li key={s.id}>
              <strong>{s.title}</strong>
              <div className="muted">
                {formatSessionWhen(s)} · {s.status}
                {s.goal_id ? ` · goal ${s.goal_id.slice(0, 8)}` : ""}
              </div>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}
