import { useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import {
  CalendarBlank,
  ChatCircle,
  CheckCircle,
  Clock,
  Sparkle,
  Target,
  WarningCircle,
} from "@phosphor-icons/react";
import { EmptyState } from "../components/EmptyState";
import { SectionHead } from "../components/SectionHead";
import {
  decideProposal,
  fetchSessions,
  fetchToday,
  fetchUsage,
  notifyCalendarChanged,
  type Session,
  type TodayResponse,
  type UsageSummary,
} from "../api";

const GOAL_BAR_COLORS = ["#60a5fa", "#34d399", "#fbbf24", "#c4b5fd", "#fb7185", "#67e8f9"];

function pad2(n: number) {
  return String(n).padStart(2, "0");
}

function formatClock(d: Date) {
  return `${pad2(d.getHours())}:${pad2(d.getMinutes())}:${pad2(d.getSeconds())}`;
}

function formatLongDate(d: Date) {
  return d.toLocaleDateString(undefined, {
    weekday: "long",
    month: "long",
    day: "numeric",
  });
}

function formatShortTime(iso: string) {
  return new Date(iso).toLocaleTimeString(undefined, {
    hour: "numeric",
    minute: "2-digit",
  });
}

function progressRatio(p: {
  completed: number;
  scheduled: number;
  missed: number;
  proposed: number;
}) {
  const total = p.completed + p.scheduled + p.missed + p.proposed;
  if (total <= 0) return 0;
  return Math.min(1, p.completed / total);
}

function barColorForRatio(ratio: number, index: number) {
  if (ratio >= 0.75) return "#34d399";
  if (ratio >= 0.4) return GOAL_BAR_COLORS[index % GOAL_BAR_COLORS.length];
  if (ratio > 0) return "#fbbf24";
  return GOAL_BAR_COLORS[index % GOAL_BAR_COLORS.length];
}

export function TodayPage() {
  const [data, setData] = useState<TodayResponse | null>(null);
  const [usage, setUsage] = useState<UsageSummary | null>(null);
  const [sessions, setSessions] = useState<Session[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [now, setNow] = useState(() => new Date());
  const [busyBatch, setBusyBatch] = useState<string | null>(null);

  useEffect(() => {
    const id = window.setInterval(() => setNow(new Date()), 1000);
    return () => window.clearInterval(id);
  }, []);

  function loadDashboard() {
    Promise.all([
      fetchToday(),
      fetchUsage().catch(() => null),
      fetchSessions().catch(() => [] as Session[]),
    ])
      .then(([today, usageRow, sessionRows]) => {
        setData(today);
        setUsage(usageRow);
        setSessions(sessionRows);
      })
      .catch((e: Error) => {
        setError(e.message);
      });
  }

  useEffect(() => {
    let cancelled = false;
    function load() {
      Promise.all([
        fetchToday(),
        fetchUsage().catch(() => null),
        fetchSessions().catch(() => [] as Session[]),
      ])
        .then(([today, usageRow, sessionRows]) => {
          if (cancelled) return;
          setData(today);
          setUsage(usageRow);
          setSessions(sessionRows);
        })
        .catch((e: Error) => {
          if (!cancelled) setError(e.message);
        });
    }
    load();
    const onCalendarChanged = () => load();
    window.addEventListener("buddy.calendar-changed", onCalendarChanged);
    return () => {
      cancelled = true;
      window.removeEventListener("buddy.calendar-changed", onCalendarChanged);
    };
  }, []);

  const upcoming = useMemo(() => {
    const source = sessions.length ? sessions : data?.todays_sessions ?? [];
    const t = now.getTime();
    return [...source]
      .filter((s) => new Date(s.start_at).getTime() >= t - 60_000)
      .filter((s) => s.status !== "missed" && s.status !== "rejected")
      .sort((a, b) => new Date(a.start_at).getTime() - new Date(b.start_at).getTime());
  }, [sessions, data?.todays_sessions, now]);

  const proposedBatches = useMemo(() => {
    const source = sessions.length ? sessions : data?.todays_sessions ?? [];
    const map = new Map<string, Session[]>();
    for (const s of source.filter((row) => row.status === "proposed")) {
      const key = s.proposal_batch_id || s.id;
      const list = map.get(key) || [];
      list.push(s);
      map.set(key, list);
    }
    return [...map.entries()].map(([batchId, rows]) => ({
      batchId,
      sessions: rows,
      title: rows[0]?.title || "Proposed sessions",
      count: rows.length,
    }));
  }, [sessions, data?.todays_sessions]);

  const proposedSessions = useMemo(
    () => proposedBatches.flatMap((b) => b.sessions),
    [proposedBatches],
  );

  async function onApproveBatch(batchId: string) {
    if (busyBatch) return;
    setBusyBatch(batchId);
    setError(null);
    try {
      await decideProposal(batchId, "approve");
      notifyCalendarChanged();
      loadDashboard();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Could not approve");
    } finally {
      setBusyBatch(null);
    }
  }

  const pendingCount =
    (data?.pending_questions.length ?? 0) +
    (data?.attention.length ?? 0) +
    proposedSessions.length;

  const todaySessionCount = data?.todays_sessions.length ?? 0;
  const activeGoals = data?.goals.length ?? 0;

  return (
    <section className="today-shell">
      <div className="today-panel today-hero">
        <div className="today-hero-copy">
          <p className="page-kicker">
            <CalendarBlank size={16} weight="duotone" />
            Today
          </p>
          <h1 className="today-date">{formatLongDate(now)}</h1>
          <p className="today-sub">Sessions, goals, and anything waiting on you.</p>
        </div>
        <div className="today-clock" aria-live="polite">
          <Clock size={22} weight="duotone" />
          <span className="today-clock-time">{formatClock(now)}</span>
        </div>
      </div>

      {error && <div className="error-banner today-error">Could not load today: {error}</div>}

      <div className="today-panel today-stats">
        <div className="today-stat-grid">
          <div className="today-stat">
            <span className="today-stat-icon" style={{ color: "#60a5fa" }}>
              <ChatCircle size={20} weight="duotone" />
            </span>
            <div>
              <div className="today-stat-value">
                {usage ? `${usage.used} / ${usage.limit}` : "—"}
              </div>
              <div className="today-stat-label">Requests asked</div>
            </div>
          </div>
          <div className="today-stat">
            <span className="today-stat-icon" style={{ color: "#34d399" }}>
              <Target size={20} weight="duotone" />
            </span>
            <div>
              <div className="today-stat-value">{data ? activeGoals : "—"}</div>
              <div className="today-stat-label">Goals</div>
            </div>
          </div>
          <div className="today-stat">
            <span className="today-stat-icon" style={{ color: "#fbbf24" }}>
              <WarningCircle size={20} weight="duotone" />
            </span>
            <div>
              <div className="today-stat-value">{data ? pendingCount : "—"}</div>
              <div className="today-stat-label">Needs you</div>
            </div>
          </div>
          <div className="today-stat">
            <span className="today-stat-icon" style={{ color: "#c4b5fd" }}>
              <CalendarBlank size={20} weight="duotone" />
            </span>
            <div>
              <div className="today-stat-value">{data ? todaySessionCount : "—"}</div>
              <div className="today-stat-label">Today</div>
            </div>
          </div>
        </div>
      </div>

      <div className="today-mid">
        <div className="today-panel today-list-panel">
          <SectionHead
            title={
              <>
                <Clock size={18} weight="duotone" />
                Upcoming
              </>
            }
            action={
              <Link className="today-link" to="/calendar">
                Calendar
              </Link>
            }
          />
          {!upcoming.length && <EmptyState>Nothing upcoming.</EmptyState>}
          <ul className="today-list">
            {upcoming.map((s) => (
              <li key={s.id} className="today-list-row">
                <div className="today-list-time">{formatShortTime(s.start_at)}</div>
                <div className="today-list-body">
                  <strong>{s.title}</strong>
                </div>
                <span className={`badge ${s.status}`}>{s.status}</span>
              </li>
            ))}
          </ul>
        </div>

        <div className="today-panel today-list-panel">
          <SectionHead
            title={
              <>
                <WarningCircle size={18} weight="duotone" />
                Needs you
              </>
            }
            action={
              <Link className="today-link" to="/chat">
                Chat
              </Link>
            }
          />
          {!pendingCount && <EmptyState>All clear.</EmptyState>}
          <ul className="today-list">
            {proposedBatches.slice(0, 4).map((batch) => (
              <li key={batch.batchId} className="today-list-row">
                <CheckCircle size={18} weight="duotone" className="today-list-icon" />
                <div className="today-list-body">
                  <strong>{batch.title}</strong>
                  <div className="muted">
                    Proposed · {batch.count} session{batch.count === 1 ? "" : "s"}
                  </div>
                </div>
                <button
                  type="button"
                  className="btn primary"
                  disabled={!!busyBatch}
                  onClick={() => onApproveBatch(batch.batchId)}
                >
                  {busyBatch === batch.batchId ? "…" : "Approve"}
                </button>
              </li>
            ))}
            {data?.attention.map((item) => (
              <li key={`att-${item}`} className="today-list-row">
                <WarningCircle size={18} weight="duotone" className="today-list-icon warn" />
                <div className="today-list-body">
                  <strong>{item}</strong>
                </div>
              </li>
            ))}
            {data?.pending_questions.map((q) => (
              <li key={`q-${q}`} className="today-list-row">
                <ChatCircle size={18} weight="duotone" className="today-list-icon" />
                <div className="today-list-body">
                  <strong>{q}</strong>
                  <div className="muted">
                    <Link to="/chat">Continue in Chat</Link>
                  </div>
                </div>
              </li>
            ))}
          </ul>
        </div>
      </div>

      <div className="today-panel today-goals">
        <SectionHead
          title={
            <>
              <Target size={18} weight="duotone" />
              Goals
            </>
          }
        />
        {!data?.goals.length && (
          <EmptyState>
            No goals yet. <Link to="/chat">Start in Chat</Link>
          </EmptyState>
        )}
        <ul className="today-goal-list">
          {data?.goals.map((g, i) => {
            const prog = data.progress?.find((p) => p.goal_id === g.id);
            const ratio = prog ? progressRatio(prog) : 0;
            const pct = Math.round(ratio * 100);
            const color = barColorForRatio(ratio, i);
            return (
              <li key={g.id} className="today-goal-row">
                <div className="today-goal-top">
                  <div>
                    <strong>{g.title}</strong>
                    <div className="muted">
                      {prog?.summary ||
                        [g.baseline && `from ${g.baseline}`, g.frequency].filter(Boolean).join(" · ") ||
                        "Clarifying the plan"}
                    </div>
                  </div>
                  <div className="today-goal-pct" style={{ color }}>
                    {pct}%
                  </div>
                </div>
                <div className="today-progress" aria-hidden>
                  <div
                    className="today-progress-fill"
                    style={{ width: `${pct}%`, background: color }}
                  />
                </div>
                {prog && (
                  <div className="today-goal-counts muted">
                    {prog.completed} done · {prog.scheduled} scheduled
                    {prog.proposed ? ` · ${prog.proposed} proposed` : ""}
                    {prog.missed ? ` · ${prog.missed} missed` : ""}
                  </div>
                )}
              </li>
            );
          })}
        </ul>
      </div>

      {data?.resurfaced_spark && (
        <div className="today-panel today-spark">
          <SectionHead
            title={
              <>
                <Sparkle size={18} weight="duotone" />
                Spark
              </>
            }
            action={
              <Link className="today-link" to="/sparks">
                Sparks
              </Link>
            }
          />
          <p className="today-spark-body">{data.resurfaced_spark.content}</p>
        </div>
      )}
    </section>
  );
}
