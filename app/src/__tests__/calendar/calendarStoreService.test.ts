import { describe, expect, it } from "vitest";
import { mergeEvents } from "@buddy/calendar/services";
import { colorForEvent } from "@buddy/calendar/utils";
import type { CalendarEvent } from "@buddy/calendar/models";

function ev(partial: Partial<CalendarEvent> & { id: string; title: string }): CalendarEvent {
  return {
    category: "general",
    start_time: 0,
    end_time: 1000,
    all_day: false,
    timezone: "UTC",
    reminders: [],
    sync_status: "local",
    created_at: 0,
    updated_at: 0,
    ...partial,
  };
}

describe("calendar helpers", () => {
  it("mergeEvents prefers incoming", () => {
    const merged = mergeEvents(
      [ev({ id: "1", title: "Old" })],
      [ev({ id: "1", title: "New" })],
    );
    expect(merged).toHaveLength(1);
    expect(merged[0].title).toBe("New");
  });

  it("colorForEvent uses category fallback", () => {
    expect(colorForEvent(ev({ id: "1", title: "X", category: "holidays" }))).toBe(
      "#F59E0B",
    );
  });
});
