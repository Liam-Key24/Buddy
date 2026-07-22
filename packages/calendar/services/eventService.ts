import type { CalendarEvent } from "../models/event";

/** Merge remote list into existing, keyed by id (optimistic UI helper). */
export function mergeEvents(
  current: CalendarEvent[],
  incoming: CalendarEvent[],
): CalendarEvent[] {
  const map = new Map<string, CalendarEvent>();
  for (const e of current) map.set(e.id, e);
  for (const e of incoming) map.set(e.id, e);
  return Array.from(map.values()).sort((a, b) => a.start_time - b.start_time);
}

export function eventsOnDay(events: CalendarEvent[], day: Date): CalendarEvent[] {
  const start = new Date(day);
  start.setHours(0, 0, 0, 0);
  const end = new Date(day);
  end.setHours(23, 59, 59, 999);
  const s = start.getTime();
  const e = end.getTime();
  return events
    .filter((ev) => ev.start_time < e && ev.end_time > s)
    .sort((a, b) => a.start_time - b.start_time);
}

export function eventsToday(events: CalendarEvent[]): CalendarEvent[] {
  return eventsOnDay(events, new Date());
}

export function upcomingEvents(
  events: CalendarEvent[],
  withinDays = 7,
): CalendarEvent[] {
  const now = Date.now();
  const until = now + withinDays * 86_400_000;
  return events
    .filter((e) => e.start_time >= now && e.start_time <= until)
    .sort((a, b) => a.start_time - b.start_time);
}
