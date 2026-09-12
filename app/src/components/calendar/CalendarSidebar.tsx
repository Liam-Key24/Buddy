import { useState } from "react";
import { MagnifyingGlass, Plus } from "@phosphor-icons/react";
import type { CalendarEvent, ScheduleKind } from "@buddy/calendar/models";
import { CATEGORIES, SCHEDULE_LAYER } from "@buddy/calendar/models";
import { useLifestyleStore } from "../../stores/useLifestyleStore";
import {
  colorForEvent,
  formatTime,
  monthGridDays,
  sameDay,
  startOfMonth,
  WEEKDAY_LETTERS,
} from "@buddy/calendar/utils";
import { eventsOnDay, upcomingEvents } from "@buddy/calendar/services";

export function CalendarSidebar({
  cursorDate,
  searchQuery,
  enabledCategories,
  showWork,
  showSleep,
  events,
  onSearch,
  onToggleCategory,
  onToggleWork,
  onToggleSleep,
  onSelectDay,
  onCreate,
  onSelectEvent,
}: {
  cursorDate: Date;
  searchQuery: string;
  enabledCategories: string[];
  showWork: boolean;
  showSleep: boolean;
  events: CalendarEvent[];
  onSearch: (q: string) => void;
  onToggleCategory: (id: string) => void;
  onToggleWork: () => void;
  onToggleSleep: () => void;
  onSelectDay: (d: Date) => void;
  onCreate: () => void;
  onSelectEvent: (id: string) => void;
}) {
  const miniDays = monthGridDays(cursorDate);
  const today = new Date();
  const month = cursorDate.getMonth();
  const todays = eventsOnDay(events, today);
  const upcoming = upcomingEvents(events, 7).slice(0, 6);

  return (
    <aside className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto [scrollbar-width:none] [-ms-overflow-style:none] [&::-webkit-scrollbar]:w-0 pr-2">
      <div className="relative">
        <MagnifyingGlass
          size={16}
          className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-zinc-500"
        />
        <input
          value={searchQuery}
          onChange={(e) => onSearch(e.target.value)}
          placeholder="Search event..."
          className="w-full rounded-xl border border-zinc-800 bg-zinc-950/60 py-2 pl-9 pr-3 text-sm text-zinc-200 outline-none transition placeholder:text-zinc-600 focus:border-blue-500/50 focus:ring-2 focus:ring-blue-500/15"
        />
      </div>

      <div>
        <div className="mb-2 text-xs font-medium text-zinc-400">
          {startOfMonth(cursorDate).toLocaleDateString(undefined, {
            month: "long",
            year: "numeric",
          })}
        </div>
        <div className="grid grid-cols-7 gap-0.5 text-center text-[10px] text-zinc-600">
          {WEEKDAY_LETTERS.map((d, i) => (
            <div key={`${d}-${i}`}>{d}</div>
          ))}
        </div>
        <div className="mt-1 grid grid-cols-7 gap-0.5">
          {miniDays.map((d) => {
            const isToday = sameDay(d, today);
            const selected = sameDay(d, cursorDate);
            const inMonth = d.getMonth() === month;
            return (
              <button
                key={d.toISOString()}
                type="button"
                onClick={() => onSelectDay(d)}
                className={`flex h-7 w-7 items-center justify-center rounded-full text-[11px] transition ${
                  isToday
                    ? "bg-blue-500 text-white"
                    : selected
                      ? "bg-zinc-800 text-zinc-100"
                      : inMonth
                        ? "text-zinc-400 hover:bg-zinc-800"
                        : "text-zinc-700"
                }`}
              >
                {d.getDate()}
              </button>
            );
          })}
        </div>
      </div>

      <div>
        <div className="mb-2 text-[10px] font-medium uppercase tracking-wider text-zinc-500">
          My calendars
        </div>
        <div className="space-y-1.5">
          {CATEGORIES.map((cat) => (
            <label
              key={cat.id}
              className="flex cursor-pointer items-center gap-2 rounded-lg px-1 py-1 text-sm text-zinc-300 hover:bg-zinc-900"
            >
              <input
                type="checkbox"
                checked={enabledCategories.includes(cat.id)}
                onChange={() => onToggleCategory(cat.id)}
                className="sr-only"
              />
              <span
                className={`flex h-3.5 w-3.5 items-center justify-center rounded border ${
                  enabledCategories.includes(cat.id)
                    ? "border-transparent"
                    : "border-zinc-600"
                }`}
                style={{
                  backgroundColor: enabledCategories.includes(cat.id)
                    ? cat.color
                    : "transparent",
                }}
              />
              {cat.label}
            </label>
          ))}
        </div>
      </div>

      <div>
        <div className="mb-2 text-[10px] font-medium uppercase tracking-wider text-zinc-500">
          Schedule layers
        </div>
        <div className="space-y-1.5">
          {(
            [
              [SCHEDULE_LAYER.work, showWork, onToggleWork],
              [SCHEDULE_LAYER.sleep, showSleep, onToggleSleep],
            ] as const
          ).map(([layer, on, toggle]) => (
            <label
              key={layer.id}
              className="flex cursor-pointer items-center gap-2 rounded-lg px-1 py-1 text-sm text-zinc-300 hover:bg-zinc-900"
            >
              <input
                type="checkbox"
                checked={on}
                onChange={toggle}
                className="sr-only"
              />
              <span
                className={`flex h-3.5 w-3.5 items-center justify-center rounded border ${
                  on ? "border-transparent" : "border-zinc-600"
                }`}
                style={{
                  backgroundColor: on ? layer.color : "transparent",
                  opacity: on ? (layer.id === "sleep" ? 0.55 : 0.9) : 1,
                }}
              />
              {layer.label}
            </label>
          ))}
        </div>
        <ScheduleHoursEditor />
      </div>

      <button
        type="button"
        onClick={onCreate}
        className="flex items-center justify-center gap-2 rounded-xl bg-blue-500 px-3 py-2.5 text-sm font-medium text-white shadow-lg shadow-blue-500/20 transition hover:bg-blue-400"
      >
        <Plus size={16} weight="bold" />
        Create Event
      </button>

      <div>
        <div className="mb-2 text-[10px] font-medium uppercase tracking-wider text-zinc-500">
          Today&apos;s agenda
        </div>
        {todays.length === 0 ? (
          <p className="text-xs text-zinc-600">Nothing scheduled</p>
        ) : (
          <ul className="space-y-1.5">
            {todays.map((ev) => (
              <li key={ev.id}>
                <button
                  type="button"
                  onClick={() => onSelectEvent(ev.id)}
                  className="flex w-full items-start gap-2 rounded-lg px-1 py-1 text-left hover:bg-zinc-900"
                >
                  <span
                    className="mt-1.5 h-2 w-2 shrink-0 rounded-full"
                    style={{ backgroundColor: colorForEvent(ev) }}
                  />
                  <div className="min-w-0">
                    <div className="truncate text-xs font-medium text-zinc-200">
                      {ev.title}
                    </div>
                    <div className="text-[10px] text-zinc-500">
                      {ev.all_day ? "All day" : formatTime(ev.start_time)}
                    </div>
                  </div>
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>

      <div className="pb-2">
        <div className="mb-2 text-[10px] font-medium uppercase tracking-wider text-zinc-500">
          Upcoming
        </div>
        {upcoming.length === 0 ? (
          <p className="text-xs text-zinc-600">No upcoming events</p>
        ) : (
          <ul className="space-y-1.5">
            {upcoming.map((ev) => (
              <li key={ev.id}>
                <button
                  type="button"
                  onClick={() => onSelectEvent(ev.id)}
                  className="flex w-full items-start gap-2 rounded-lg px-1 py-1 text-left hover:bg-zinc-900"
                >
                  <span
                    className="mt-1.5 h-2 w-2 shrink-0 rounded-full"
                    style={{ backgroundColor: colorForEvent(ev) }}
                  />
                  <div className="min-w-0">
                    <div className="truncate text-xs font-medium text-zinc-200">
                      {ev.title}
                    </div>
                    <div className="text-[10px] text-zinc-500">
                      {new Date(ev.start_time).toLocaleDateString(undefined, {
                        month: "short",
                        day: "numeric",
                      })}
                      {!ev.all_day ? ` · ${formatTime(ev.start_time)}` : ""}
                    </div>
                  </div>
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </aside>
  );
}

function typicalHm(
  rules: { kind: ScheduleKind; segments: { start_hm: string; end_hm: string }[] }[],
  kind: ScheduleKind,
): { start: string; end: string } | null {
  const seg = rules.find((r) => r.kind === kind)?.segments[0];
  if (!seg) return null;
  return { start: seg.start_hm, end: seg.end_hm };
}

function ScheduleHoursEditor() {
  const saveHours = useLifestyleStore((s) => s.saveHours);
  const rules = useLifestyleStore((s) => s.scheduleRules);
  const [editing, setEditing] = useState<ScheduleKind | null>(null);
  const [start, setStart] = useState("");
  const [end, setEnd] = useState("");
  const [saving, setSaving] = useState(false);

  const work = typicalHm(rules, "work");
  const sleep = typicalHm(rules, "sleep");

  async function save() {
    if (!editing) return;
    setSaving(true);
    try {
      await saveHours(editing, start, end);
      setEditing(null);
    } finally {
      setSaving(false);
    }
  }

  function begin(kind: ScheduleKind, current: { start: string; end: string } | null) {
    setEditing(kind);
    setStart(current?.start ?? (kind === "work" ? "09:00" : "22:30"));
    setEnd(current?.end ?? (kind === "work" ? "17:00" : "07:45"));
  }

  return (
    <div className="mt-3 space-y-2 rounded-lg border border-zinc-800/80 px-2 py-2">
      <div className="text-[10px] font-medium uppercase tracking-wider text-zinc-500">
        Usual hours
      </div>
      {(
        [
          ["work", "Work", work],
          ["sleep", "Sleep", sleep],
        ] as const
      ).map(([kind, label, times]) => (
        <div key={kind} className="text-xs text-zinc-300">
          {editing === kind ? (
            <div className="flex flex-wrap items-center gap-1.5">
              <span className="w-10 shrink-0 text-zinc-400">{label}</span>
              <input
                type="time"
                value={start}
                onChange={(e) => setStart(e.target.value)}
                className="rounded-md border border-zinc-700 bg-zinc-950 px-1.5 py-0.5 text-[11px] text-zinc-200"
              />
              <span className="text-zinc-600">–</span>
              <input
                type="time"
                value={end}
                onChange={(e) => setEnd(e.target.value)}
                className="rounded-md border border-zinc-700 bg-zinc-950 px-1.5 py-0.5 text-[11px] text-zinc-200"
              />
              <button
                type="button"
                disabled={saving}
                onClick={() => void save()}
                className="rounded-md bg-blue-500 px-1.5 py-0.5 text-[11px] font-medium text-white hover:bg-blue-400 disabled:opacity-50"
              >
                Save
              </button>
              <button
                type="button"
                onClick={() => setEditing(null)}
                className="text-[11px] text-zinc-500 hover:text-zinc-300"
              >
                Cancel
              </button>
            </div>
          ) : (
            <button
              type="button"
              onClick={() => begin(kind, times)}
              className="flex w-full items-center justify-between rounded-md px-0.5 py-0.5 text-left hover:bg-zinc-900"
            >
              <span className="text-zinc-400">{label}</span>
              <span className="tabular-nums text-zinc-200">
                {times ? `${times.start}–${times.end}` : "Set hours"}
              </span>
            </button>
          )}
        </div>
      ))}
    </div>
  );
}
