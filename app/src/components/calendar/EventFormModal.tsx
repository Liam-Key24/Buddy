import { useState } from "react";
import { X } from "@phosphor-icons/react";
import type {
  CreateEventInput,
  RecurrenceRule,
  Reminder,
} from "@buddy/calendar/models";
import { CATEGORIES } from "@buddy/calendar/models";
import {
  fromLocalInputValue,
  toDateInputValue,
  toLocalInputValue,
} from "@buddy/calendar/utils";

export interface EventFormInitial {
  title: string;
  description?: string | null;
  location?: string | null;
  category?: string;
  color?: string | null;
  start_time: number;
  end_time: number;
  all_day?: boolean;
  timezone: string;
  reminders?: Reminder[];
  recurrence?: RecurrenceRule | null;
}

export function EventFormModal({
  mode,
  initial,
  onClose,
  onSubmit,
}: {
  mode: "create" | "edit";
  initial: EventFormInitial;
  onClose: () => void;
  onSubmit: (input: CreateEventInput) => Promise<void>;
}) {
  const [title, setTitle] = useState(initial.title);
  const [description, setDescription] = useState(initial.description ?? "");
  const [location, setLocation] = useState(initial.location ?? "");
  const [category, setCategory] = useState(initial.category ?? "general");
  const [allDay, setAllDay] = useState(initial.all_day ?? false);
  const [start, setStart] = useState(toLocalInputValue(initial.start_time));
  const [end, setEnd] = useState(toLocalInputValue(initial.end_time));
  const [dateOnly, setDateOnly] = useState(toDateInputValue(initial.start_time));
  const [timezone, setTimezone] = useState(initial.timezone);
  const [reminderMinutes, setReminderMinutes] = useState(
    String(initial.reminders?.[0]?.minutes_before ?? 15),
  );
  const [extraReminder, setExtraReminder] = useState(
    initial.reminders?.[1] ? String(initial.reminders[1].minutes_before) : "",
  );
  const [recurrenceFreq, setRecurrenceFreq] = useState(
    initial.recurrence?.frequency ?? "",
  );
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    let startMs: number;
    let endMs: number;
    if (allDay) {
      const d = new Date(dateOnly + "T00:00:00");
      startMs = d.getTime();
      endMs = startMs + 86_400_000 - 1;
    } else {
      startMs = fromLocalInputValue(start);
      endMs = fromLocalInputValue(end);
    }
    if (!title.trim()) {
      setError("Title is required");
      return;
    }
    if (!(endMs > startMs)) {
      setError("End must be after start");
      return;
    }
    const reminders: Reminder[] = [
      {
        minutes_before: Number(reminderMinutes) || 15,
        method: "popup",
      },
    ];
    if (extraReminder.trim()) {
      reminders.push({
        minutes_before: Number(extraReminder) || 60,
        method: "popup",
      });
    }
    const recurrence: RecurrenceRule | null = recurrenceFreq
      ? { frequency: recurrenceFreq, interval: 1, by_day: [] }
      : null;

    setSaving(true);
    setError(null);
    try {
      await onSubmit({
        title: title.trim(),
        description: description.trim() || null,
        location: location.trim() || null,
        category,
        start_time: startMs,
        end_time: endMs,
        all_day: allDay,
        timezone,
        reminders,
        recurrence,
      });
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setSaving(false);
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4 backdrop-blur-sm">
      <form
        onSubmit={handleSubmit}
        className="w-full max-w-md rounded-2xl border border-zinc-800 bg-zinc-900 p-5 shadow-2xl shadow-black/40"
      >
        <div className="mb-4 flex items-center justify-between">
          <h3 className="text-base font-semibold text-zinc-100">
            {mode === "create" ? "New event" : "Edit event"}
          </h3>
          <button
            type="button"
            onClick={onClose}
            className="rounded-lg p-1 text-zinc-500 hover:bg-zinc-800 hover:text-zinc-300"
          >
            <X size={18} />
          </button>
        </div>

        <div className="space-y-3">
          <input
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder="Event name"
            className="w-full rounded-xl border border-zinc-700 bg-zinc-950/60 px-3 py-2.5 text-sm text-zinc-100 outline-none focus:border-blue-500"
            autoFocus
          />
          {allDay ? (
            <input
              type="date"
              value={dateOnly}
              onChange={(e) => setDateOnly(e.target.value)}
              className="w-full rounded-xl border border-zinc-700 bg-zinc-950/60 px-3 py-2 text-sm text-zinc-200 outline-none focus:border-blue-500"
            />
          ) : (
            <div className="grid grid-cols-[1fr_auto_1fr] items-center gap-2">
              <input
                type="datetime-local"
                value={start}
                onChange={(e) => setStart(e.target.value)}
                className="rounded-xl border border-zinc-700 bg-zinc-950/60 px-2 py-2 text-sm text-zinc-200 outline-none focus:border-blue-500"
              />
              <span className="text-zinc-600">→</span>
              <input
                type="datetime-local"
                value={end}
                onChange={(e) => setEnd(e.target.value)}
                className="rounded-xl border border-zinc-700 bg-zinc-950/60 px-2 py-2 text-sm text-zinc-200 outline-none focus:border-blue-500"
              />
            </div>
          )}
          <label className="flex items-center gap-2 text-sm text-zinc-400">
            <input
              type="checkbox"
              checked={allDay}
              onChange={(e) => setAllDay(e.target.checked)}
              className="rounded border-zinc-600"
            />
            All day
          </label>
          <select
            value={category}
            onChange={(e) => setCategory(e.target.value)}
            className="w-full rounded-xl border border-zinc-700 bg-zinc-950/60 px-3 py-2 text-sm text-zinc-200 outline-none focus:border-blue-500"
          >
            {CATEGORIES.map((c) => (
              <option key={c.id} value={c.id}>
                {c.label}
              </option>
            ))}
          </select>
          <input
            value={location}
            onChange={(e) => setLocation(e.target.value)}
            placeholder="Location"
            className="w-full rounded-xl border border-zinc-700 bg-zinc-950/60 px-3 py-2 text-sm text-zinc-200 outline-none focus:border-blue-500"
          />
          <textarea
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            placeholder="Description"
            rows={2}
            className="w-full resize-none rounded-xl border border-zinc-700 bg-zinc-950/60 px-3 py-2 text-sm text-zinc-200 outline-none focus:border-blue-500"
          />
          <div className="grid grid-cols-2 gap-2">
            <div>
              <label className="mb-1 block text-[10px] uppercase tracking-wider text-zinc-500">
                Reminder (min)
              </label>
              <input
                value={reminderMinutes}
                onChange={(e) => setReminderMinutes(e.target.value)}
                className="w-full rounded-xl border border-zinc-700 bg-zinc-950/60 px-3 py-2 text-sm text-zinc-200 outline-none focus:border-blue-500"
              />
            </div>
            <div>
              <label className="mb-1 block text-[10px] uppercase tracking-wider text-zinc-500">
                2nd reminder
              </label>
              <input
                value={extraReminder}
                onChange={(e) => setExtraReminder(e.target.value)}
                placeholder="optional"
                className="w-full rounded-xl border border-zinc-700 bg-zinc-950/60 px-3 py-2 text-sm text-zinc-200 outline-none focus:border-blue-500"
              />
            </div>
          </div>
          <div className="grid grid-cols-2 gap-2">
            <div>
              <label className="mb-1 block text-[10px] uppercase tracking-wider text-zinc-500">
                Repeat
              </label>
              <select
                value={recurrenceFreq}
                onChange={(e) => setRecurrenceFreq(e.target.value)}
                className="w-full rounded-xl border border-zinc-700 bg-zinc-950/60 px-3 py-2 text-sm text-zinc-200 outline-none focus:border-blue-500"
              >
                <option value="">Does not repeat</option>
                <option value="DAILY">Daily</option>
                <option value="WEEKLY">Weekly</option>
                <option value="MONTHLY">Monthly</option>
                <option value="YEARLY">Yearly</option>
              </select>
            </div>
            <div>
              <label className="mb-1 block text-[10px] uppercase tracking-wider text-zinc-500">
                Timezone
              </label>
              <input
                value={timezone}
                onChange={(e) => setTimezone(e.target.value)}
                className="w-full rounded-xl border border-zinc-700 bg-zinc-950/60 px-3 py-2 text-sm text-zinc-200 outline-none focus:border-blue-500"
              />
            </div>
          </div>
          {error && <p className="text-xs text-rose-400">{error}</p>}
        </div>

        <div className="mt-5 flex justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            className="rounded-xl px-4 py-2 text-sm text-zinc-400 hover:text-zinc-200"
          >
            Cancel
          </button>
          <button
            type="submit"
            disabled={saving}
            className="rounded-xl bg-blue-500 px-4 py-2 text-sm font-medium text-white hover:bg-blue-400 disabled:opacity-50"
          >
            {saving ? "Saving…" : "Save"}
          </button>
        </div>
      </form>
    </div>
  );
}
