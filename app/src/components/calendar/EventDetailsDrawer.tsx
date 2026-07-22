import {
  Copy,
  MapPin,
  PencilSimple,
  Trash,
  X,
} from "@phosphor-icons/react";
import type { CalendarEvent } from "@buddy/calendar/models";
import { CATEGORIES } from "@buddy/calendar/models";
import {
  colorForEvent,
  formatDayHeader,
  formatTimeRange,
} from "@buddy/calendar/utils";

export function EventDetailsDrawer({
  event,
  onClose,
  onEdit,
  onDuplicate,
  onDelete,
}: {
  event: CalendarEvent | null;
  onClose: () => void;
  onEdit: () => void;
  onDuplicate: () => void;
  onDelete: () => void;
}) {
  if (!event) return null;

  const category =
    CATEGORIES.find((c) => c.id === event.category)?.label ?? event.category;

  return (
    <div className="fixed inset-y-0 right-0 z-40 flex w-full max-w-sm flex-col border-l border-zinc-800 bg-zinc-900 shadow-2xl shadow-black/40 animate-[slideIn_0.2s_ease-out]">
      <div className="flex items-center justify-between border-b border-zinc-800 px-4 py-3">
        <h3 className="text-sm font-semibold text-zinc-100">Event details</h3>
        <button
          type="button"
          onClick={onClose}
          className="rounded-lg p-1 text-zinc-500 hover:bg-zinc-800 hover:text-zinc-300"
        >
          <X size={18} />
        </button>
      </div>
      <div className="flex-1 overflow-y-auto p-4">
        <div className="mb-3 flex items-start gap-3">
          <span
            className="mt-1 h-3 w-3 shrink-0 rounded-full"
            style={{ backgroundColor: colorForEvent(event) }}
          />
          <div>
            <h2 className="text-lg font-semibold text-zinc-100">{event.title}</h2>
            <p className="mt-1 text-sm text-zinc-400">
              {formatDayHeader(new Date(event.start_time))}
            </p>
            <p className="text-sm text-zinc-500">
              {formatTimeRange(event.start_time, event.end_time, event.all_day)}
            </p>
          </div>
        </div>
        <dl className="space-y-3 text-sm">
          <div>
            <dt className="text-[10px] uppercase tracking-wider text-zinc-500">
              Category
            </dt>
            <dd className="text-zinc-300">{category}</dd>
          </div>
          {event.location && (
            <div>
              <dt className="text-[10px] uppercase tracking-wider text-zinc-500">
                Location
              </dt>
              <dd className="flex items-center gap-1.5 text-zinc-300">
                <MapPin size={14} className="text-zinc-500" />
                {event.location}
              </dd>
            </div>
          )}
          {event.description && (
            <div>
              <dt className="text-[10px] uppercase tracking-wider text-zinc-500">
                Description
              </dt>
              <dd className="whitespace-pre-wrap text-zinc-300">
                {event.description}
              </dd>
            </div>
          )}
          {event.recurrence && (
            <div>
              <dt className="text-[10px] uppercase tracking-wider text-zinc-500">
                Repeats
              </dt>
              <dd className="text-zinc-300">
                {event.recurrence.frequency.toLowerCase()}
              </dd>
            </div>
          )}
          {event.reminders.length > 0 && (
            <div>
              <dt className="text-[10px] uppercase tracking-wider text-zinc-500">
                Reminders
              </dt>
              <dd className="text-zinc-300">
                {event.reminders
                  .map((r) => `${r.minutes_before} min before`)
                  .join(", ")}
              </dd>
            </div>
          )}
          <div>
            <dt className="text-[10px] uppercase tracking-wider text-zinc-500">
              Timezone
            </dt>
            <dd className="text-zinc-300">{event.timezone}</dd>
          </div>
        </dl>
      </div>
      <div className="flex gap-2 border-t border-zinc-800 p-4">
        <button
          type="button"
          onClick={onEdit}
          className="flex flex-1 items-center justify-center gap-1.5 rounded-xl border border-zinc-700 py-2 text-xs font-medium text-zinc-300 hover:bg-zinc-800"
        >
          <PencilSimple size={14} /> Edit
        </button>
        <button
          type="button"
          onClick={onDuplicate}
          className="flex flex-1 items-center justify-center gap-1.5 rounded-xl border border-zinc-700 py-2 text-xs font-medium text-zinc-300 hover:bg-zinc-800"
        >
          <Copy size={14} /> Duplicate
        </button>
        <button
          type="button"
          onClick={onDelete}
          className="flex flex-1 items-center justify-center gap-1.5 rounded-xl border border-rose-500/30 py-2 text-xs font-medium text-rose-400 hover:bg-rose-500/10"
        >
          <Trash size={14} /> Delete
        </button>
      </div>
    </div>
  );
}
