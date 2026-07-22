import type { CalendarEvent } from "../models/event";

export function filterEventsByQuery(
  events: CalendarEvent[],
  query: string,
): CalendarEvent[] {
  const q = query.trim().toLowerCase();
  if (!q) return events;
  return events.filter(
    (e) =>
      e.title.toLowerCase().includes(q) ||
      (e.description?.toLowerCase().includes(q) ?? false) ||
      (e.location?.toLowerCase().includes(q) ?? false),
  );
}
