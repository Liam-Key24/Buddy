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

/** Convert `#RGB` / `#RRGGBB` to `rgba(r,g,b,a)`. Falls back to the input if unparseable. */
export function withAlpha(hex: string, alpha: number): string {
  const raw = hex.trim().replace(/^#/, "");
  let r = 0;
  let g = 0;
  let b = 0;
  if (raw.length === 3) {
    r = parseInt(raw[0] + raw[0], 16);
    g = parseInt(raw[1] + raw[1], 16);
    b = parseInt(raw[2] + raw[2], 16);
  } else if (raw.length === 6) {
    r = parseInt(raw.slice(0, 2), 16);
    g = parseInt(raw.slice(2, 4), 16);
    b = parseInt(raw.slice(4, 6), 16);
  } else {
    return hex;
  }
  if ([r, g, b].some((n) => Number.isNaN(n))) return hex;
  const a = Math.min(1, Math.max(0, alpha));
  return `rgba(${r}, ${g}, ${b}, ${a})`;
}
