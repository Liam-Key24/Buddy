import type { CalendarEvent } from "@buddy/calendar/models";
import { eventsOnDay } from "@buddy/calendar/services";
import { colorForEvent, monthGridDays, sameDay } from "@buddy/calendar/utils";

const WEEKDAYS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

export function MonthView({
  cursorDate,
  events,
  selectedEventId,
  onSelectDay,
  onSelectEvent,
  onCreateAt,
}: {
  cursorDate: Date;
  events: CalendarEvent[];
  selectedEventId: string | null;
  onSelectDay: (d: Date) => void;
  onSelectEvent: (id: string) => void;
  onCreateAt: (d: Date) => void;
}) {
  const days = monthGridDays(cursorDate);
  const today = new Date();
  const month = cursorDate.getMonth();

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-2xl border border-zinc-800 bg-zinc-950/40">
      <div className="grid grid-cols-7 border-b border-zinc-800">
        {WEEKDAYS.map((d) => (
          <div
            key={d}
            className="px-2 py-2 text-center text-[11px] font-medium uppercase tracking-wider text-zinc-500"
          >
            {d}
          </div>
        ))}
      </div>
      <div className="grid min-h-0 flex-1 grid-cols-7 grid-rows-6">
        {days.map((day) => {
          const inMonth = day.getMonth() === month;
          const isToday = sameDay(day, today);
          const dayEvents = eventsOnDay(events, day).slice(0, 3);
          const extra = eventsOnDay(events, day).length - dayEvents.length;
          return (
            <div
              key={day.toISOString()}
              className={`group flex min-h-0 flex-col border-b border-r border-zinc-800/80 p-1.5 transition hover:bg-zinc-900/50 ${
                inMonth ? "" : "bg-zinc-950/30"
              }`}
              onDoubleClick={() => onCreateAt(day)}
            >
              <button
                type="button"
                onClick={() => onSelectDay(day)}
                className={`mb-1 flex h-7 w-7 items-center justify-center self-start rounded-full text-xs font-medium transition ${
                  isToday
                    ? "bg-blue-500 text-white"
                    : inMonth
                      ? "text-zinc-300 hover:bg-zinc-800"
                      : "text-zinc-600"
                }`}
              >
                {day.getDate()}
              </button>
              <div className="flex min-h-0 flex-1 flex-col gap-0.5 overflow-hidden">
                {dayEvents.map((ev) => (
                  <button
                    key={ev.id}
                    type="button"
                    onClick={(e) => {
                      e.stopPropagation();
                      onSelectEvent(ev.id);
                    }}
                    className={`truncate rounded-md px-1.5 py-0.5 text-left text-[10px] font-medium text-white/95 transition ${
                      selectedEventId === ev.id ? "ring-1 ring-white/40" : ""
                    }`}
                    style={{ backgroundColor: colorForEvent(ev) }}
                    title={ev.title}
                  >
                    {ev.title}
                  </button>
                ))}
                {extra > 0 && (
                  <span className="px-1 text-[10px] text-zinc-500">
                    +{extra} more
                  </span>
                )}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
