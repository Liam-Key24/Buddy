import type { ScheduleBlock } from "@buddy/calendar/models";
import { SCHEDULE_LAYER } from "@buddy/calendar/models";
import type { CalendarEvent } from "@buddy/calendar/models";
import { colorForEvent, formatTime, sameDay, withAlpha } from "@buddy/calendar/utils";
import { eventsOnDay } from "@buddy/calendar/services";

const HOURS = Array.from({ length: 24 }, (_, i) => i);
const PX_PER_HOUR = 48;

/** Clip a block to a single local calendar day for painting. */
function segmentOnDay(
  block: ScheduleBlock,
  day: Date,
): { top: number; height: number } | null {
  const dayStart = new Date(day);
  dayStart.setHours(0, 0, 0, 0);
  const dayEnd = new Date(dayStart);
  dayEnd.setDate(dayEnd.getDate() + 1);

  const start = Math.max(block.start_time, dayStart.getTime());
  const end = Math.min(block.end_time, dayEnd.getTime());
  if (end <= start) return null;

  const startDate = new Date(start);
  const top =
    startDate.getHours() * PX_PER_HOUR +
    (startDate.getMinutes() / 60) * PX_PER_HOUR;
  const height = Math.max(
    8,
    ((end - start) / 3_600_000) * PX_PER_HOUR,
  );
  return { top, height };
}

export function TimeGridView({
  days,
  events,
  scheduleBlocks = [],
  selectedEventId,
  selectedBlockId,
  onSelectEvent,
  onSelectBlock,
  onCreateAt,
}: {
  days: Date[];
  events: CalendarEvent[];
  scheduleBlocks?: ScheduleBlock[];
  selectedEventId: string | null;
  selectedBlockId?: string | null;
  onSelectEvent: (id: string) => void;
  onSelectBlock?: (id: string) => void;
  onCreateAt: (day: Date, hour: number) => void;
}) {
  const today = new Date();

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-2xl border border-zinc-800 bg-zinc-950/40">
      <div
        className="grid border-b border-zinc-800"
        style={{ gridTemplateColumns: `56px repeat(${days.length}, 1fr)` }}
      >
        <div />
        {days.map((d) => (
          <div
            key={d.toISOString()}
            className={`px-2 py-2 text-center text-xs ${
              sameDay(d, today) ? "text-blue-400" : "text-zinc-400"
            }`}
          >
            <div className="font-medium">
              {d.toLocaleDateString(undefined, { weekday: "short" })}
            </div>
            <div
              className={`mx-auto mt-1 flex h-7 w-7 items-center justify-center rounded-full text-sm ${
                sameDay(d, today) ? "bg-blue-500 text-white" : "text-zinc-200"
              }`}
            >
              {d.getDate()}
            </div>
          </div>
        ))}
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto">
        <div
          className="grid"
          style={{
            gridTemplateColumns: `56px repeat(${days.length}, 1fr)`,
            minHeight: 24 * PX_PER_HOUR,
          }}
        >
          <div className="relative">
            {HOURS.map((h) => (
              <div
                key={h}
                className="absolute right-2 text-[10px] text-zinc-600"
                style={{ top: h * PX_PER_HOUR - 6 }}
              >
                {h === 0
                  ? ""
                  : `${h % 12 || 12}${h < 12 ? "am" : "pm"}`}
              </div>
            ))}
          </div>
          {days.map((day) => {
            const dayEvents = eventsOnDay(events, day).filter((e) => !e.all_day);
            const allDay = eventsOnDay(events, day).filter((e) => e.all_day);
            return (
              <div
                key={day.toISOString()}
                className="relative border-l border-zinc-800/80"
              >
                {HOURS.map((h) => (
                  <button
                    key={h}
                    type="button"
                    className="absolute left-0 right-0 border-t border-zinc-800/50 hover:bg-zinc-900/40"
                    style={{ top: h * PX_PER_HOUR, height: PX_PER_HOUR }}
                    onDoubleClick={() => onCreateAt(day, h)}
                    aria-label={`Create at ${h}:00`}
                  />
                ))}
                {scheduleBlocks.map((block) => {
                  const seg = segmentOnDay(block, day);
                  if (!seg) return null;
                  const layer = SCHEDULE_LAYER[block.kind];
                  const selected = selectedBlockId === block.id;
                  return (
                    <button
                      key={`${block.id}-${day.toISOString()}`}
                      type="button"
                      onClick={(e) => {
                        e.stopPropagation();
                        onSelectBlock?.(block.id);
                      }}
                      className={`absolute left-0.5 right-0.5 overflow-hidden rounded-md border-l-[3px] px-1.5 py-1 text-left text-[10px] transition ${
                        selected
                          ? "ring-2 ring-white/35 ring-offset-1 ring-offset-zinc-950"
                          : "hover:brightness-110"
                      }`}
                      style={{
                        top: seg.top,
                        height: seg.height,
                        zIndex: 1,
                        backgroundColor: withAlpha(
                          layer.color,
                          selected ? 0.28 : 0.16,
                        ),
                        color: withAlpha(layer.color, 0.95),
                        borderLeftColor: layer.color,
                      }}
                      title={`${block.title} · protected`}
                    >
                      <div className="truncate font-medium text-zinc-100/90">
                        {block.title}
                      </div>
                      <div className="truncate text-[9px] uppercase tracking-wide text-zinc-400">
                        Protected
                      </div>
                    </button>
                  );
                })}
                {allDay.map((ev, i) => {
                  const accent = colorForEvent(ev);
                  const selected = selectedEventId === ev.id;
                  return (
                    <button
                      key={ev.id}
                      type="button"
                      onClick={(e) => {
                        e.stopPropagation();
                        onSelectEvent(ev.id);
                      }}
                      className={`absolute left-1 right-1 truncate rounded-md border-l-[3px] px-1.5 py-0.5 text-left text-[10px] font-medium text-zinc-100 ${
                        selected
                          ? "ring-2 ring-white/40 ring-offset-1 ring-offset-zinc-950"
                          : ""
                      }`}
                      style={{
                        top: 2 + i * 18,
                        backgroundColor: withAlpha(
                          accent,
                          selected ? 0.35 : 0.22,
                        ),
                        borderLeftColor: accent,
                        zIndex: 2,
                      }}
                    >
                      {ev.title}
                    </button>
                  );
                })}
                {dayEvents.map((ev) => {
                  const start = new Date(ev.start_time);
                  const end = new Date(ev.end_time);
                  const top =
                    start.getHours() * PX_PER_HOUR +
                    (start.getMinutes() / 60) * PX_PER_HOUR;
                  const height = Math.max(
                    24,
                    ((end.getTime() - start.getTime()) / 3_600_000) * PX_PER_HOUR,
                  );
                  const accent = colorForEvent(ev);
                  const selected = selectedEventId === ev.id;
                  return (
                    <button
                      key={ev.id}
                      type="button"
                      onClick={(e) => {
                        e.stopPropagation();
                        onSelectEvent(ev.id);
                      }}
                      className={`absolute left-1 right-1 overflow-hidden rounded-lg border-l-[3px] px-1.5 py-1 text-left text-[11px] font-medium text-zinc-100 shadow-sm transition ${
                        selected
                          ? "ring-2 ring-white/45 ring-offset-1 ring-offset-zinc-950"
                          : "hover:brightness-110"
                      }`}
                      style={{
                        top,
                        height,
                        backgroundColor: withAlpha(
                          accent,
                          selected ? 0.38 : 0.24,
                        ),
                        borderLeftColor: accent,
                        zIndex: 3,
                      }}
                      title={`${ev.title} · ${formatTime(ev.start_time)}`}
                    >
                      <div className="truncate">{ev.title}</div>
                      <div className="truncate text-[10px] text-zinc-300/90">
                        {formatTime(ev.start_time)}
                      </div>
                    </button>
                  );
                })}
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
