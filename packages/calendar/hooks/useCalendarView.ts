import type { CalendarView } from "../models/event";
import { addDays, addMonths, visibleRangeForView } from "../utils/date";

export function shiftCursor(
  cursor: Date,
  view: CalendarView,
  dir: -1 | 1,
): Date {
  if (view === "month") return addMonths(cursor, dir);
  if (view === "week") return addDays(cursor, dir * 7);
  return addDays(cursor, dir);
}

export function rangeFor(view: CalendarView, cursor: Date) {
  return visibleRangeForView(view, cursor);
}
