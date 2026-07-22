import type { CalendarEvent } from "@buddy/calendar/models";
import {
  colorForEvent,
  formatDayHeader,
  formatTimeRange,
  startOfDay,
} from "@buddy/calendar/utils";

export function AgendaView({
  events,
  selectedEventId,
  onSelectEvent,
}: {
  events: CalendarEvent[];
  selectedEventId: string | null;
  onSelectEvent: (id: string) => void;
}) {
  const groups = new Map<number, CalendarEvent[]>();
  for (const ev of events) {
    const key = startOfDay(new Date(ev.start_time));
    const list = groups.get(key) ?? [];
    list.push(ev);
    groups.set(key, list);
  }
  const sortedKeys = Array.from(groups.keys()).sort((a, b) => a - b);

  if (sortedKeys.length === 0) {
    return (
      <div className="flex min-h-0 flex-1 items-center justify-center rounded-2xl border border-zinc-800 bg-zinc-950/40 text-sm text-zinc-500">
        No upcoming events in this range
      </div>
    );
  }

  return (
    <div className="min-h-0 flex-1 space-y-4 overflow-y-auto rounded-2xl border border-zinc-800 bg-zinc-950/40 p-4">
      {sortedKeys.map((key) => (
        <div key={key}>
          <h3 className="mb-2 text-xs font-medium uppercase tracking-wider text-zinc-500">
            {formatDayHeader(new Date(key))}
          </h3>
          <div className="space-y-1.5">
            {(groups.get(key) ?? []).map((ev) => (
              <button
                key={ev.id}
                type="button"
                onClick={() => onSelectEvent(ev.id)}
                className={`flex w-full items-start gap-3 rounded-xl border px-3 py-2.5 text-left transition ${
                  selectedEventId === ev.id
                    ? "border-blue-500/40 bg-blue-500/10"
                    : "border-zinc-800 bg-zinc-900/50 hover:border-zinc-700"
                }`}
              >
                <span
                  className="mt-1 h-2.5 w-2.5 shrink-0 rounded-full"
                  style={{ backgroundColor: colorForEvent(ev) }}
                />
                <div className="min-w-0 flex-1">
                  <div className="truncate text-sm font-medium text-zinc-200">
                    {ev.title}
                  </div>
                  <div className="text-xs text-zinc-500">
                    {formatTimeRange(ev.start_time, ev.end_time, ev.all_day)}
                    {ev.location ? ` · ${ev.location}` : ""}
                  </div>
                </div>
              </button>
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}
