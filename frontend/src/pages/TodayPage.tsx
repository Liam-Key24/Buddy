import { useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import {
  CalendarBlank,
  CaretDown,
  CaretRight,
  ChatCircle,
  CheckCircle,
  Clock,
  Sparkle,
  Target,
  WarningCircle,
} from "@phosphor-icons/react";
import { CategoryIcon } from "../components/CategoryIcon";
import { EmptyState } from "../components/ui/EmptyState";
import { SectionHead } from "../components/ui/SectionHead";
import { Skeleton } from "../components/ui/Skeleton";
import { Surface } from "../components/ui/Surface";
import { Tag } from "../components/ui/Tag";
import { Button } from "../components/ui/Button";
import {
  decideProposal,
  fetchSessions,
  fetchToday,
  fetchUsage,
  notifyCalendarChanged,
  type Session,
  type TodayNeed,
  type TodayResponse,
  type UsageSummary,
} from "../api";
import { useChatNav } from "../chatNav";
import { useNavigate } from "react-router-dom";
import { useToast } from "../components/ui/Toast";

const CONTINUE_HINT_KEY = "buddy.continueHint";
/** Upcoming shows today through the next 2 calendar days (3 days total). */
const UPCOMING_DAY_COUNT = 3;

const GOAL_BAR_COLORS = ["#eaf6cb", "#9dde9a", "#e8c56b", "#c5d9a0", "#f0a0a0", "#a8c5a0"];

type UpcomingDay = {
  key: string;
  label: string;
  rows: UpcomingRow[];
};

type UpcomingRow =
  | { kind: "session"; session: Session }
  | {
      kind: "group";
      key: string;
      label: string;
      iconName: string;
      color?: string;
      sessions: Session[];
      status: string;
    };

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

function startOfLocalDay(d: Date) {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate());
}

function dayKey(d: Date) {
  return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`;
}

function dayLabel(day: Date, today: Date) {
  const diffDays = Math.round(
    (startOfLocalDay(day).getTime() - startOfLocalDay(today).getTime()) / 86_400_000,
  );
  if (diffDays === 0) return "Today";
  if (diffDays === 1) return "Tomorrow";
  return day.toLocaleDateString(undefined, { weekday: "short", month: "short", day: "numeric" });
}

/** Strip trailing "· Session N" so similar titles collapse together. */
function titleBase(title: string) {
  return title.replace(/\s*[·•\-–]\s*session\s*\d+\s*$/i, "").trim() || title;
}

function groupKeyFor(s: Session) {
  if (s.category_id) return `cat:${s.category_id}`;
  if (s.category?.name) return `name:${s.category.name.toLowerCase()}`;
  return `title:${titleBase(s.title).toLowerCase()}`;
}

function groupLabelFor(s: Session) {
  return s.category?.name || titleBase(s.title);
}

function groupStatus(sessions: Session[]) {
  const statuses = new Set(sessions.map((s) => s.status));
  if (statuses.size === 1) return sessions[0].status;
  if (statuses.has("proposed")) return "proposed";
  return "scheduled";
}

function formatGroupTime(sessions: Session[]) {
  return formatShortTime(sessions[0].start_at);
}

function buildUpcomingDays(sessions: Session[], now: Date): UpcomingDay[] {
  const todayStart = startOfLocalDay(now);
  const horizonEnd = new Date(todayStart);
  horizonEnd.setDate(horizonEnd.getDate() + UPCOMING_DAY_COUNT);
  const nowMs = now.getTime() - 60_000;

  const inWindow = sessions
    .filter((s) => {
      const start = new Date(s.start_at);
      if (Number.isNaN(start.getTime())) return false;
      if (start.getTime() < nowMs) return false;
      if (start >= horizonEnd) return false;
      if (s.status === "missed" || s.status === "rejected") return false;
      return true;
    })
    .sort((a, b) => new Date(a.start_at).getTime() - new Date(b.start_at).getTime());

  const byDay = new Map<string, Session[]>();
  for (const s of inWindow) {
    const start = new Date(s.start_at);
    const key = dayKey(start);
    const list = byDay.get(key);
    if (list) list.push(s);
    else byDay.set(key, [s]);
  }

  const days: UpcomingDay[] = [];
  for (let offset = 0; offset < UPCOMING_DAY_COUNT; offset++) {
    const day = new Date(todayStart);
    day.setDate(day.getDate() + offset);
    const key = dayKey(day);
    const daySessions = byDay.get(key);
    if (!daySessions?.length) continue;

    const buckets = new Map<string, Session[]>();
    for (const s of daySessions) {
      const gk = groupKeyFor(s);
      const list = buckets.get(gk);
      if (list) list.push(s);
      else buckets.set(gk, [s]);
    }

    const rows: UpcomingRow[] = [];
    for (const [gk, group] of buckets) {
      if (group.length === 1) {
        rows.push({ kind: "session", session: group[0] });
        continue;
      }
      const head = group[0];
      rows.push({
        kind: "group",
        key: `${key}:${gk}`,
        label: groupLabelFor(head),
        iconName: head.category?.icon || head.category?.name || groupLabelFor(head),
        color: head.category?.color || undefined,
        sessions: group,
        status: groupStatus(group),
      });
    }

    rows.sort((a, b) => {
      const aStart = a.kind === "session" ? a.session.start_at : a.sessions[0].start_at;
      const bStart = b.kind === "session" ? b.session.start_at : b.sessions[0].start_at;
      return aStart.localeCompare(bStart);
    });

    days.push({ key, label: dayLabel(day, todayStart), rows });
  }

  return days;
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
  if (ratio >= 0.75) return "#9dde9a";
  if (ratio >= 0.4) return GOAL_BAR_COLORS[index % GOAL_BAR_COLORS.length];
  if (ratio > 0) return "#e8c56b";
  return GOAL_BAR_COLORS[index % GOAL_BAR_COLORS.length];
}

export function TodayPage() {
  const navigate = useNavigate();
  const { conversations, setConversationId } = useChatNav();
  const { pushToast } = useToast();
  const [data, setData] = useState<TodayResponse | null>(null);
  const [usage, setUsage] = useState<UsageSummary | null>(null);
  const [sessions, setSessions] = useState<Session[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [now, setNow] = useState(() => new Date());
  const [busyBatch, setBusyBatch] = useState<string | null>(null);
  const [expandedGroups, setExpandedGroups] = useState<Record<string, boolean>>({});
  const loading = !data && !error;

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

  const upcomingDays = useMemo(() => {
    const source = sessions.length ? sessions : data?.todays_sessions ?? [];
    return buildUpcomingDays(source, now);
  }, [sessions, data?.todays_sessions, now]);

  const hasUpcoming = upcomingDays.some((d) => d.rows.length > 0);

  function toggleGroup(key: string) {
    setExpandedGroups((prev) => ({ ...prev, [key]: !prev[key] }));
  }

  const needs = useMemo(() => {
    if (data?.needs?.length) return data.needs;
    return [] as TodayNeed[];
  }, [data?.needs]);

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

  function continueNeed(need: TodayNeed) {
    const inSidebar = conversations.some((c) => c.id === need.conversation_id);
    if (!inSidebar) {
      pushToast("That chat was deleted — Needs you will clear on refresh");
      loadDashboard();
      return;
    }
    setConversationId(need.conversation_id);
    localStorage.setItem(
      CONTINUE_HINT_KEY,
      JSON.stringify({
        title: need.title,
        detail: need.detail,
        kind: need.kind,
        at: Date.now(),
      }),
    );
    navigate("/chat");
  }

  const pendingCount = needs.length;

  const todaySessionCount = data?.todays_sessions.length ?? 0;
  const activeGoals = data?.goals.length ?? 0;

  const stats = [
    {
      icon: <ChatCircle size={20} weight="duotone" />,
      value: usage ? `${usage.used} / ${usage.limit}` : "—",
      label: "Requests",
      color: "text-mint",
    },
    {
      icon: <Target size={20} weight="duotone" />,
      value: data ? String(activeGoals) : "—",
      label: "Goals",
      color: "text-ok",
    },
    {
      icon: <WarningCircle size={20} weight="duotone" />,
      value: data ? String(pendingCount) : "—",
      label: "Needs you",
      color: "text-warn",
    },
    {
      icon: <CalendarBlank size={20} weight="duotone" />,
      value: data ? String(todaySessionCount) : "—",
      label: "Today",
      color: "text-mint-dim",
    },
  ];

  return (
    <section className="h-full overflow-y-auto p-5">
      <div className="mb-4 flex flex-wrap items-end justify-between gap-3">
        <div>
          <p className="m-0 flex items-center gap-1.5 text-xs tracking-wide text-muted uppercase">
            <CalendarBlank size={14} weight="duotone" />
            Today
          </p>
          <h1 className="mt-1 mb-0 font-display text-3xl font-medium">{formatLongDate(now)}</h1>
        </div>
        <div className="flex items-center gap-2 text-mint" aria-live="polite">
          <Clock size={20} weight="duotone" />
          <span className="font-display text-xl tabular-nums">{formatClock(now)}</span>
        </div>
      </div>

      {error && (
        <div className="mb-4 rounded-card bg-danger/15 px-3 py-2 text-sm text-danger">
          Could not load today: {error}
        </div>
      )}

      <div className="mb-4 grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
        {stats.map((s) => (
          <Surface key={s.label} className="flex items-center gap-3">
            <span className={s.color}>{s.icon}</span>
            <div>
              {loading ? (
                <Skeleton className="mb-1 h-6 w-16" />
              ) : (
                <div className="text-xl font-semibold">{s.value}</div>
              )}
              <div className="text-xs text-muted">{s.label}</div>
            </div>
          </Surface>
        ))}
      </div>

      <div className="mb-4 grid gap-3 lg:grid-cols-2">
        <Surface>
          <SectionHead
            icon={<Clock size={16} weight="duotone" />}
            title="Upcoming"
            action={
              <Link className="text-xs text-mint no-underline hover:underline" to="/calendar">
                Calendar
              </Link>
            }
          />
          {loading && (
            <div className="flex flex-col gap-2">
              <Skeleton className="h-10" />
              <Skeleton className="h-10" />
            </div>
          )}
          {!loading && !hasUpcoming && (
            <EmptyState
              icon={<Clock size={22} />}
              action={
                <Link to="/calendar" className="text-mint">
                  Open calendar
                </Link>
              }
            >
              Nothing in the next 3 days.
            </EmptyState>
          )}
          <div className="flex flex-col gap-3">
            {upcomingDays.map((day) => (
              <div key={day.key}>
                <p className="m-0 mb-1 px-1 text-[11px] font-medium tracking-wide text-muted uppercase">
                  {day.label}
                </p>
                <ul className="m-0 flex list-none flex-col gap-1 p-0">
                  {day.rows.map((row) => {
                    if (row.kind === "session") {
                      const s = row.session;
                      return (
                        <li key={s.id}>
                          <Link
                            to="/calendar"
                            className="flex items-center gap-3 rounded-xl px-1 py-1.5 text-ink no-underline hover:bg-raised-soft"
                          >
                            <div className="w-14 shrink-0 text-xs text-muted">
                              {formatShortTime(s.start_at)}
                            </div>
                            <strong className="min-w-0 flex-1 truncate text-sm font-medium">
                              {s.title}
                            </strong>
                            <Tag tone={s.status === "proposed" ? "warn" : "mint"}>{s.status}</Tag>
                          </Link>
                        </li>
                      );
                    }

                    const open = !!expandedGroups[row.key];
                    return (
                      <li key={row.key}>
                        <button
                          type="button"
                          onClick={() => toggleGroup(row.key)}
                          className="flex w-full items-center gap-3 rounded-xl px-1 py-1.5 text-left text-ink hover:bg-raised-soft"
                        >
                          <div className="w-14 shrink-0 text-xs text-muted">
                            {formatGroupTime(row.sessions)}
                          </div>
                          <span
                            className="flex min-w-0 flex-1 items-center gap-1.5"
                            style={row.color ? { color: row.color } : undefined}
                          >
                            {open ? (
                              <CaretDown size={12} className="shrink-0 text-muted" />
                            ) : (
                              <CaretRight size={12} className="shrink-0 text-muted" />
                            )}
                            <CategoryIcon name={row.iconName} size={14} />
                            <strong className="min-w-0 truncate text-sm font-medium text-ink">
                              {row.label}
                              <span className="font-normal text-muted">
                                {" "}
                                · {row.sessions.length} sessions
                              </span>
                            </strong>
                          </span>
                          <Tag
                            tone={row.status === "proposed" ? "warn" : "mint"}
                            color={row.color}
                          >
                            {row.status}
                          </Tag>
                        </button>
                        {open && (
                          <ul className="m-0 mt-0.5 mb-1 ml-3 flex list-none flex-col gap-0.5 border-l border-hairline py-0.5 pl-2">
                            {row.sessions.map((s) => (
                              <li key={s.id}>
                                <Link
                                  to="/calendar"
                                  className="flex items-center gap-3 rounded-lg px-1 py-1 text-ink no-underline hover:bg-raised-soft"
                                >
                                  <div className="w-14 shrink-0 text-xs text-muted">
                                    {formatShortTime(s.start_at)}
                                  </div>
                                  <span className="min-w-0 flex-1 truncate text-xs text-ink-soft">
                                    {s.title}
                                  </span>
                                  <Tag tone={s.status === "proposed" ? "warn" : "mint"}>
                                    {s.status}
                                  </Tag>
                                </Link>
                              </li>
                            ))}
                          </ul>
                        )}
                      </li>
                    );
                  })}
                </ul>
              </div>
            ))}
          </div>
        </Surface>

        <Surface>
          <SectionHead
            icon={<WarningCircle size={16} weight="duotone" />}
            title="Needs you"
            action={
              <Link className="text-xs text-mint no-underline hover:underline" to="/chat">
                Chat
              </Link>
            }
          />
          {loading && <Skeleton className="h-16" />}
          {!loading && !pendingCount && (
            <EmptyState icon={<CheckCircle size={22} />}>All clear.</EmptyState>
          )}
          <ul className="m-0 flex list-none flex-col gap-2 p-0">
            {needs.map((need) => (
              <li key={need.id} className="flex items-start gap-2">
                <WarningCircle size={18} weight="duotone" className="mt-0.5 text-warn" />
                <div className="min-w-0 flex-1">
                  <strong className="text-sm">{need.title}</strong>
                  {need.detail && <div className="text-xs text-muted">{need.detail}</div>}
                  <div className="mt-1 flex flex-wrap gap-2">
                    {need.kind === "approve" && need.proposal_batch_id && (
                      <Button
                        tone="primary"
                        disabled={!!busyBatch}
                        onClick={() => onApproveBatch(need.proposal_batch_id!)}
                      >
                        {busyBatch === need.proposal_batch_id ? "…" : "Approve"}
                      </Button>
                    )}
                    <button
                      type="button"
                      className="text-xs text-mint underline"
                      onClick={() => continueNeed(need)}
                    >
                      Continue in Chat
                    </button>
                  </div>
                </div>
              </li>
            ))}
          </ul>
        </Surface>
      </div>

      <Surface className="mb-4">
        <SectionHead icon={<Target size={16} weight="duotone" />} title="Goals" />
        {loading && (
          <div className="flex flex-col gap-3">
            <Skeleton className="h-12" />
            <Skeleton className="h-12" />
          </div>
        )}
        {!loading && !data?.goals.length && (
          <EmptyState
            icon={<Target size={22} />}
            action={
              <Link to="/chat" className="text-mint">
                Start in Chat
              </Link>
            }
          >
            No goals yet.
          </EmptyState>
        )}
        <ul className="m-0 flex list-none flex-col gap-3 p-0">
          {data?.goals.map((g, i) => {
            const prog = data.progress?.find((p) => p.goal_id === g.id);
            const ratio = prog ? progressRatio(prog) : 0;
            const pct = Math.round(ratio * 100);
            const color = barColorForRatio(ratio, i);
            return (
              <li key={g.id}>
                <Link to="/chat" className="block text-ink no-underline">
                  <div className="mb-1 flex items-center justify-between gap-2">
                    <div>
                      <strong className="text-sm">{g.title}</strong>
                      <div className="text-xs text-muted">
                        {prog?.summary ||
                          [g.baseline && `from ${g.baseline}`, g.frequency].filter(Boolean).join(" · ") ||
                          "Clarifying the plan"}
                      </div>
                    </div>
                    <div className="text-sm font-semibold" style={{ color }}>
                      {pct}%
                    </div>
                  </div>
                  <div className="h-1.5 overflow-hidden rounded-pill bg-page-deep" aria-hidden>
                    <div
                      className="h-full rounded-pill"
                      style={{ width: `${pct}%`, background: color }}
                    />
                  </div>
                  {prog && (
                    <div className="mt-1 text-xs text-muted">
                      {prog.completed} done · {prog.scheduled} scheduled
                      {prog.proposed ? ` · ${prog.proposed} proposed` : ""}
                      {prog.missed ? ` · ${prog.missed} missed` : ""}
                    </div>
                  )}
                </Link>
              </li>
            );
          })}
        </ul>
      </Surface>

      {data?.resurfaced_spark && (
        <Surface>
          <SectionHead
            icon={<Sparkle size={16} weight="duotone" />}
            title="Spark"
            action={
              <Link className="text-xs text-mint no-underline hover:underline" to="/sparks">
                Sparks
              </Link>
            }
          />
          <p className="m-0 text-sm text-ink-soft">{data.resurfaced_spark.content}</p>
        </Surface>
      )}
    </section>
  );
}
