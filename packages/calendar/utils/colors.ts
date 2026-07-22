import type { CalendarEvent } from "../models/event";
import { CATEGORIES } from "../models/event";

export function colorForCategory(category: string): string {
  return (
    CATEGORIES.find((c) => c.id === category)?.color ??
    CATEGORIES.find((c) => c.id === "general")!.color
  );
}

export function colorForEvent(event: CalendarEvent): string {
  if (event.color) return event.color;
  return colorForCategory(event.category);
}
