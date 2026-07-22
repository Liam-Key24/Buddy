import { MagnifyingGlass, Plus } from "@phosphor-icons/react";
import type { CalendarEvent } from "@buddy/calendar/models";
import { CATEGORIES, SCHEDULE_LAYER } from "@buddy/calendar/models";
import {
  colorForEvent,
  formatTime,
  monthGridDays,
  sameDay,
  startOfMonth,
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
    <aside className="flex w-64 shrink-0 flex-col gap-4 overflow-y-auto border-r border-zinc-800 pr-4">
      <div>
        <h2 className="mb-3 text-lg font-semibold tracking-tight text-zinc-100">
          Calendar
        </h2>
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
      </div>

      <div>
        <div className="mb-2 text-xs font-medium text-zinc-400">
          {startOfMonth(cursorDate).toLocaleDateString(undefined, {
            month: "long",
            year: "numeric",
          })}
        </div>
        <div className="grid grid-cols-7 gap-0.5 text-center text-[10px] text-zinc-600">
          {["S", "M", "T", "W", "T", "F", "S"].map((d, i) => (
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
