import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { fetchToday, formatSessionWhen, type TodayResponse } from "../api";

export function TodayPage() {
  const [data, setData] = useState<TodayResponse | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    fetchToday()
      .then((d) => {
        if (!cancelled) setData(d);
      })
      .catch((e: Error) => {
        if (!cancelled) setError(e.message);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <section>
      <h1 className="page-title">Today</h1>
      <p className="page-sub">
        Agreed sessions, active goals, and anything waiting for your decision.
      </p>

      {error && <p className="muted">Could not load today: {error}</p>}

      <div className="panel">
        <h2>Today’s sessions</h2>
        {!data?.todays_sessions?.length && <p className="muted">No sessions for today.</p>}
        <ul className="list">
          {data?.todays_sessions?.map((s) => (
            <li key={s.id}>
              <strong>{s.title}</strong>
              <div className="muted">
                {formatSessionWhen(s)} · {s.status}
              </div>
            </li>
          ))}
        </ul>
      </div>

      <div className="panel">
        <h2>Active goals</h2>
        {!data?.goals.length && <p className="muted">No active goals yet. Start in Chat.</p>}
        <ul className="list">
          {data?.goals.map((g) => {
            const prog = data.progress?.find((p) => p.goal_id === g.id);
            return (
              <li key={g.id}>
                <strong>{g.title}</strong>
                <div className="muted">
                  {[g.baseline && `from ${g.baseline}`, g.frequency, g.status]
                    .filter(Boolean)
                    .join(" · ")}
                  {prog
                    ? ` · ${prog.completed} done / ${prog.missed} missed / ${prog.scheduled} scheduled`
                    : ""}
                </div>
              </li>
            );
          })}
        </ul>
      </div>

      <div className="panel">
        <h2>Needs attention</h2>
        {!data?.attention.length && <p className="muted">Nothing waiting.</p>}
        <ul className="list">
          {data?.attention.map((item) => (
            <li key={item}>{item}</li>
          ))}
        </ul>
      </div>

      <div className="panel">
        <h2>Pending questions</h2>
        {!data?.pending_questions.length && <p className="muted">No open questions.</p>}
        <ul className="list">
          {data?.pending_questions.map((q) => (
            <li key={q}>
              {q}{" "}
              <Link to="/chat">Continue in Chat</Link>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}
