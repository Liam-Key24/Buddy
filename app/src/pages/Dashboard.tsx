import {
  BookOpen,
  Brain,
  CaretLeft,
  CaretRight,
  CheckSquare,
  ChatsCircle,
  Code,
  Cpu,
  FileText,
  Lightning,
  ShareNetwork,
  Sparkle,
  Wallet,
  type Icon,
} from "@phosphor-icons/react";
import { useConversationStore } from "../stores/useConversationStore";
import { useAppStore, type AppPage } from "../stores/useAppStore";
import { useSettingsStore } from "../stores/useSettingsStore";
import { fetchServiceStatus, loadSettings } from "../lib/api";
import {
  lifeDashboardSnapshot,
  lifeDashboardWeek,
  type LifeSnapshot,
  type StudyFocus,
  type TaskDay,
} from "../lib/lifeApi";
import { shortDate } from "../lib/dates";
import { useEffect, useState } from "react";
import { createPortal } from "react-dom";

function formatMoney(cents: number) {
  const n = cents / 100;
  const abs = Math.abs(n).toFixed(0);
  return n < 0 ? `−£${abs}` : `£${abs}`;
}

function topicProgress(status: string) {
  if (status === "completed") return 1;
  if (status === "in_progress") return 0.55;
  return 0.08;
}

export function Dashboard() {
  const { conversations } = useConversationStore();
  const { mlxStatus, brainStatus, setMlxStatus, setBrainStatus, setCurrentPage } =
    useAppStore();
  const settings = useSettingsStore();
  const [snap, setSnap] = useState<LifeSnapshot | null>(null);
  const [weekOffset, setWeekOffset] = useState(0);
  const [weekDays, setWeekDays] = useState<TaskDay[]>([]);
  const [weekLabel, setWeekLabel] = useState("week");

  useEffect(() => {
    loadSettings().then((s) => {
      settings.setSettings({
        mlxUrl: s.mlx_url,
        brainUrl: s.brain_url,
        modelName: s.model_name,
        logLevel: s.log_level,
        autoStartMlx: s.auto_start_mlx,
      });
    });
    lifeDashboardSnapshot()
      .then((s) => {
        setSnap(s);
        setWeekDays(s.task_week ?? []);
        setWeekLabel(s.week_label ?? "week");
        setWeekOffset(0);
      })
      .catch(() => setSnap(null));
    fetchServiceStatus()
      .then((status) => {
        setMlxStatus(status.mlx ? "online" : "offline");
        setBrainStatus(status.brain ? "online" : "offline");
      })
      .catch(() => {
        setMlxStatus("offline");
        setBrainStatus("offline");
      });
  }, [setMlxStatus, setBrainStatus]);

  const chats = conversations.filter((c) => c.kind !== "codex");
  const codeChats = conversations.filter((c) => c.kind === "codex");
  const eaten = snap?.calories_eaten ?? 0;
  const target = snap?.calorie_target || 1;
  const kcalLeft = snap != null ? Math.round(target - eaten) : null;
  const calOver = kcalLeft != null && kcalLeft < 0;
  const todosDone = snap?.todos_completed ?? 0;
  const todosTotal = todosDone + (snap?.open_todos ?? 0);
  const mlxOn = mlxStatus === "online";
  const brainOn = brainStatus === "online";

  return (
    <div className="flex-1 overflow-y-auto p-5">
      <div className="dashboard-stagger mx-auto flex max-w-5xl flex-col gap-3">
        <div className="grid grid-cols-3 gap-3">
          <button
            type="button"
            title="Settings"
            onClick={() => setCurrentPage("settings")}
            className="flex items-center justify-center gap-4 rounded-2xl border border-zinc-800 bg-zinc-900 p-4 transition hover:border-zinc-700"
          >
            <Cpu
              size={22}
              weight="duotone"
              className={mlxOn ? "text-zinc-100" : "text-zinc-700"}
            />
            <Brain
              size={22}
              weight="duotone"
              className={brainOn ? "text-zinc-100" : "text-zinc-700"}
            />
            <Sparkle
              size={22}
              weight="duotone"
              className={mlxOn ? "text-zinc-100" : "text-zinc-700"}
            />
          </button>
          <CountTile
            icon={ChatsCircle}
            value={chats.length}
            tone="text-sky-400"
            onClick={() => setCurrentPage("chat")}
          />
          <CountTile
            icon={Lightning}
            value={snap?.sparks_active ?? 0}
            tone="text-amber-400"
            onClick={() => setCurrentPage("spark")}
          />
        </div>

        <div className="grid grid-cols-1 gap-3 sm:grid-cols-12">
          <TaskWeekCard
            days={weekDays}
            label={weekLabel}
            onOpen={() => setCurrentPage("calendar")}
            onPrev={() => {
              const next = weekOffset - 1;
              setWeekOffset(next);
              lifeDashboardWeek(next)
                .then((w) => {
                  setWeekDays(w.task_week ?? []);
                  setWeekLabel(w.week_label);
                })
                .catch(console.error);
            }}
            onNext={() => {
              const next = weekOffset + 1;
              setWeekOffset(next);
              lifeDashboardWeek(next)
                .then((w) => {
                  setWeekDays(w.task_week ?? []);
                  setWeekLabel(w.week_label);
                })
                .catch(console.error);
            }}
          />
          <button
            type="button"
            onClick={() => setCurrentPage("todo")}
            className="flex flex-col rounded-2xl border border-zinc-800 bg-zinc-900 p-4 text-left transition hover:border-zinc-700 sm:col-span-5"
          >
            <div className="flex items-center justify-between">
              <CheckSquare
                size={20}
                weight="duotone"
                className={snap && snap.overdue_todos > 0 ? "text-rose-400" : "text-orange-400"}
              />
              {todosTotal > 0 && (
                <span className="text-[11px] tabular-nums text-zinc-500">
                  {todosDone}/{todosTotal}
                </span>
              )}
            </div>
            <div className="mt-3 space-y-2">
              {(snap?.open_todo_preview ?? []).length === 0 ? (
                <p className="text-sm text-zinc-600">—</p>
              ) : (
                (snap?.open_todo_preview ?? []).map((t, i) => (
                  <div key={`${t.title}-${i}`} className="min-w-0">
                    <p className="truncate text-sm text-zinc-100">{t.title}</p>
                    {t.overdue && <p className="text-[11px] text-rose-400">overdue</p>}
                  </div>
                ))
              )}
            </div>
          </button>
        </div>

        <div className="grid grid-cols-1 gap-3 sm:grid-cols-12">
          <button
            type="button"
            onClick={() => setCurrentPage("fitness")}
            className="flex min-h-[10.5rem] items-center justify-center rounded-2xl border border-zinc-800 bg-zinc-900 p-3 transition hover:border-zinc-700 sm:col-span-5"
          >
            <ArcProgress
              value={eaten / target}
              color={calOver ? "#fb7185" : "#34d399"}
              label={kcalLeft != null ? String(Math.abs(kcalLeft)) : "—"}
              caption={calOver ? "over" : "left"}
            />
          </button>
          <StudyCard focus={snap?.study_focus ?? null} onOpen={() => setCurrentPage("study")} />
          <button
            type="button"
            onClick={() => setCurrentPage("money")}
            className="flex min-h-[10.5rem] flex-col items-center justify-center gap-2 rounded-2xl border border-zinc-800 bg-zinc-900 p-4 transition hover:border-zinc-700 sm:col-span-3"
          >
            <Wallet size={18} weight="duotone" className="self-start text-teal-400" />
            <MoneyCircle
              income={snap?.money_income_cents ?? 0}
              expense={snap?.money_expense_cents ?? 0}
              net={snap?.money_net_cents ?? null}
            />
          </button>
        </div>

        <div className="grid grid-cols-3 gap-3">
          <ToolButton
            icon={FileText}
            value={snap?.docs_count ?? 0}
            tone="text-zinc-400"
            page="documents"
            onOpen={setCurrentPage}
          />
          <ToolButton
            icon={ShareNetwork}
            value={snap?.social_drafts ?? 0}
            tone="text-fuchsia-400"
            page="socials"
            onOpen={setCurrentPage}
          />
          <ToolButton
            icon={Code}
            value={codeChats.length}
            tone="text-violet-400"
            page="code"
            onOpen={setCurrentPage}
          />
        </div>
      </div>
    </div>
  );
}

function CountTile({
  icon: TileIcon,
  value,
  tone,
  onClick,
}: {
  icon: Icon;
  value: number;
  tone: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex items-center justify-between rounded-2xl border border-zinc-800 bg-zinc-900 p-4 text-left transition hover:border-zinc-700"
    >
      <TileIcon size={22} weight="duotone" className={tone} />
      <span className="text-2xl font-semibold tabular-nums tracking-tight text-zinc-100">
        {value}
      </span>
    </button>
  );
}

function ToolButton({
  icon: TileIcon,
  value,
  tone,
  page,
  onOpen,
}: {
  icon: Icon;
  value: number;
  tone: string;
  page: AppPage;
  onOpen: (page: AppPage) => void;
}) {
  return (
    <button
      type="button"
      onClick={() => onOpen(page)}
      className="flex items-center justify-between rounded-2xl border border-zinc-800 bg-zinc-900 px-4 py-3.5 text-left transition hover:border-zinc-700"
    >
      <TileIcon size={20} weight="duotone" className={tone} />
      <span className="text-lg font-semibold tabular-nums text-zinc-100">{value}</span>
    </button>
  );
}

function ArcProgress({
  value,
  color,
  label,
  caption,
}: {
  value: number;
  color: string;
  label: string;
  caption: string;
}) {
  const w = 220;
  const stroke = 14;
  const r = 78;
  const cx = w / 2;
  const cy = 88;
  const h = 124;
  const pct = Math.min(1, Math.max(0, Number.isFinite(value) ? value : 0));
  const start = polar(cx, cy, r, 200);
  const end = polar(cx, cy, r, -20);
  const d = `M ${start.x} ${start.y} A ${r} ${r} 0 1 1 ${end.x} ${end.y}`;

  return (
    <div className="relative w-full max-w-md">
      <svg viewBox={`0 0 ${w} ${h}`} className="h-auto w-full" aria-hidden>
        <path
          d={d}
          fill="none"
          stroke="#3f3f46"
          strokeWidth={stroke}
          strokeLinecap="round"
        />
        <path
          d={d}
          fill="none"
          stroke={color}
          strokeWidth={stroke}
          strokeLinecap="round"
          pathLength={100}
          strokeDasharray={`${pct * 100} 100`}
        />
      </svg>
      <div className="pointer-events-none absolute inset-x-0 top-[40%] flex flex-col items-center">
        <span className="text-2xl font-semibold tabular-nums tracking-tight text-zinc-100">
          {label}
        </span>
        <span className="text-[11px] text-zinc-500">{caption}</span>
      </div>
    </div>
  );
}

function polar(cx: number, cy: number, r: number, deg: number) {
  const rad = (deg * Math.PI) / 180;
  return { x: cx + r * Math.cos(rad), y: cy - r * Math.sin(rad) };
}

function TaskWeekCard({
  days,
  label,
  onOpen,
  onPrev,
  onNext,
}: {
  days: TaskDay[];
  label: string;
  onOpen: () => void;
  onPrev: () => void;
  onNext: () => void;
}) {
  const [tip, setTip] = useState<{ x: number; y: number; titles: string[] } | null>(
    null,
  );
  const max = Math.max(1, ...days.map((d) => d.titles.length));
  const today = new Date().toISOString().slice(0, 10);

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onOpen}
      onKeyDown={(e) => {
        if (e.target !== e.currentTarget) return;
        if (e.key === "Enter" || e.key === " ") onOpen();
      }}
      className="relative cursor-pointer rounded-2xl border border-zinc-800 bg-zinc-900 p-4 sm:col-span-7"
    >
      <div className="mb-3 flex items-center justify-between gap-2">
        <p className="text-xs text-zinc-500">{label}</p>
        <div className="flex items-center gap-0.5">
          <button
            type="button"
            title="Previous week"
            aria-label="Previous week"
            onClick={(e) => {
              e.stopPropagation();
              onPrev();
            }}
            className="rounded-md p-1 text-zinc-500 hover:bg-zinc-800 hover:text-zinc-200"
          >
            <CaretLeft size={14} weight="bold" />
          </button>
          <button
            type="button"
            title="Next week"
            aria-label="Next week"
            onClick={(e) => {
              e.stopPropagation();
              onNext();
            }}
            className="rounded-md p-1 text-zinc-500 hover:bg-zinc-800 hover:text-zinc-200"
          >
            <CaretRight size={14} weight="bold" />
          </button>
        </div>
      </div>
      <div className="flex h-40 items-end gap-2">
        {(days.length ? days : placeholderWeek()).map((d) => {
          const count = d.titles.length;
          const h = count === 0 ? 8 : Math.max(20, Math.round((count / max) * 100));
          return (
            <div key={d.date} className="flex min-w-0 flex-1 flex-col items-center gap-1.5">
              <div className="flex h-28 w-full items-end">
                <div
                  className={`w-full rounded-t-md ${
                    count === 0
                      ? "bg-zinc-800"
                      : d.date === today
                        ? "bg-emerald-400"
                        : "bg-emerald-400/70"
                  }`}
                  style={{ height: `${h}%` }}
                  onMouseMove={(e) => {
                    e.stopPropagation();
                    setTip({ x: e.clientX, y: e.clientY, titles: d.titles });
                  }}
                  onMouseLeave={() => setTip(null)}
                />
              </div>
              <span className="text-[10px] uppercase text-zinc-600">{d.label.slice(0, 1)}</span>
            </div>
          );
        })}
      </div>
      {tip &&
        createPortal(
          <div
            className="pointer-events-none fixed z-[200] max-w-48 rounded-lg border border-zinc-700 bg-zinc-950 px-2.5 py-1.5 text-xs text-zinc-200 shadow-lg"
            style={{
              left: Math.min(tip.x + 12, window.innerWidth - 200),
              top: Math.min(tip.y + 12, window.innerHeight - 120),
            }}
          >
            {tip.titles.length === 0 ? (
              <span className="text-zinc-600">—</span>
            ) : (
              <ul className="space-y-0.5">
                {tip.titles.map((name, i) => (
                  <li key={`${name}-${i}`} className="truncate">
                    {name}
                  </li>
                ))}
              </ul>
            )}
          </div>,
          document.body,
        )}
    </div>
  );
}

function placeholderWeek(): TaskDay[] {
  return ["M", "T", "W", "T", "F", "S", "S"].map((label, i) => ({
    date: `empty-${i}`,
    label,
    titles: [],
  }));
}

function StudyCard({
  focus,
  onOpen,
}: {
  focus: StudyFocus | null;
  onOpen: () => void;
}) {
  const pct = focus ? topicProgress(focus.status) : 0;
  const risk =
    focus?.risk === "unlikely"
      ? "text-rose-400"
      : focus?.risk === "at_risk"
        ? "text-amber-400"
        : "text-emerald-400";

  return (
    <button
      type="button"
      onClick={onOpen}
      className="flex min-h-[10.5rem] flex-col rounded-2xl border border-zinc-800 bg-zinc-900 p-4 text-left transition hover:border-zinc-700 sm:col-span-4"
    >
      <BookOpen size={20} weight="duotone" className="text-indigo-400" />
      {focus ? (
        <>
          <p className="mt-3 truncate text-sm font-medium text-zinc-100">{focus.name}</p>
          <p className="mt-1 text-[11px] capitalize text-zinc-500">
            {focus.status.replace("_", " ")}
            {focus.remaining_estimate != null ? ` · ${focus.remaining_estimate}h` : ""}
          </p>
          <div className="mt-3 h-1.5 overflow-hidden rounded-full bg-zinc-800">
            <div
              className="h-full rounded-full bg-indigo-400"
              style={{ width: `${Math.round(pct * 100)}%` }}
            />
          </div>
          <div className="mt-3 flex items-center justify-between text-[11px] text-zinc-500">
            <span>{shortDate(focus.deadline) ?? shortDate(focus.last_studied) ?? "—"}</span>
            <span className={risk}>{focus.risk?.replace("_", " ") ?? ""}</span>
          </div>
        </>
      ) : (
        <p className="mt-6 text-sm text-zinc-600">—</p>
      )}
    </button>
  );
}

function MoneyCircle({
  income,
  expense,
  net,
}: {
  income: number;
  expense: number;
  net: number | null;
}) {
  const total = Math.max(income + expense, 1);
  const inc = income / total;
  const c = 2 * Math.PI * 45;
  const green = c * inc;
  const red = c - green;

  return (
    <div className="relative h-24 w-24">
      <svg viewBox="0 0 100 100" className="h-full w-full -rotate-90" aria-hidden>
        <circle
          cx="50"
          cy="50"
          r="45"
          fill="none"
          stroke="#34d399"
          strokeWidth="3.5"
          strokeDasharray={`${green} ${c}`}
          strokeLinecap="butt"
        />
        <circle
          cx="50"
          cy="50"
          r="42"
          fill="none"
          stroke="#fb7185"
          strokeWidth="3.5"
          strokeDasharray={`${red} ${c}`}
          strokeDashoffset={-green}
          strokeLinecap="butt"
        />
      </svg>
      <span className="absolute inset-0 flex items-center justify-center text-xs font-semibold tabular-nums text-zinc-100">
        {net != null ? formatMoney(net) : "—"}
      </span>
    </div>
  );
}
