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
      <p className="page-sub">Agreed sessions, active goals, and anything waiting for your decision.</p>

      {error && <div className="error-banner">Could not load today: {error}</div>}

      <div className="panel">
        <h2>Today’s sessions</h2>
        {!data?.todays_sessions?.length && (
          <p className="empty-state">No sessions for today.</p>
        )}
        <ul className="list">
          {data?.todays_sessions?.map((s) => (
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

      <div className="panel">
        <h2>Active goals</h2>
        {!data?.goals.length && (
          <p className="empty-state">
            No active goals yet. <Link to="/chat">Start in Chat</Link>
          </p>
        )}
        <ul className="list">
          {data?.goals.map((g) => {
            const prog = data.progress?.find((p) => p.goal_id === g.id);
            return (
              <li key={g.id} className="goal-row">
                <div>
                  <strong>{g.title}</strong>
                  <div className="muted">
                    {prog?.summary ||
                      [g.baseline && `from ${g.baseline}`, g.frequency].filter(Boolean).join(" · ") ||
                      "Getting clear on the plan"}
                  </div>
                </div>
              </li>
            );
          })}
        </ul>
      </div>

      <div className="panel">
        <h2>Needs attention</h2>
        {!data?.attention.length && <p className="empty-state">Nothing waiting.</p>}
        <ul className="list">
          {data?.attention.map((item) => (
            <li key={item}>{item}</li>
          ))}
        </ul>
      </div>

      <div className="panel">
        <h2>Pending</h2>
        {!data?.pending_questions.length && <p className="empty-state">No open questions.</p>}
        <ul className="list">
          {data?.pending_questions.map((q) => (
            <li key={q}>
              {q} · <Link to="/chat">Continue in Chat</Link>
            </li>
          ))}
        </ul>
      </div>

      {data?.resurfaced_spark && (
        <div className="panel">
          <h2>Spark</h2>
          <p style={{ margin: 0 }}>{data.resurfaced_spark.content}</p>
          <p className="muted" style={{ margin: "0.45rem 0 0" }}>
            <Link to="/sparks">Open Sparks</Link>
          </p>
        </div>
      )}
    </section>
  );
}
