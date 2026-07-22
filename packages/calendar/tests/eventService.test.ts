import { describe, expect, it } from "vitest";
import { mergeEvents, eventsOnDay } from "../services/eventService";
import { colorForCategory, colorForEvent } from "../utils/colors";
import type { CalendarEvent } from "../models/event";

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

describe("mergeEvents", () => {
  it("merges by id and sorts by start", () => {
    const a = ev({ id: "1", title: "A", start_time: 200 });
    const b = ev({ id: "2", title: "B", start_time: 100 });
    const updated = ev({ id: "1", title: "A2", start_time: 200 });
    const merged = mergeEvents([a, b], [updated]);
    expect(merged.map((e) => e.id)).toEqual(["2", "1"]);
    expect(merged[1].title).toBe("A2");
  });
});

describe("eventsOnDay", () => {
  it("filters events overlapping the day", () => {
    const day = new Date(2026, 6, 19, 12);
    const on = ev({
      id: "1",
      title: "On",
      start_time: new Date(2026, 6, 19, 10).getTime(),
      end_time: new Date(2026, 6, 19, 11).getTime(),
    });
    const off = ev({
      id: "2",
      title: "Off",
      start_time: new Date(2026, 6, 20, 10).getTime(),
      end_time: new Date(2026, 6, 20, 11).getTime(),
    });
    expect(eventsOnDay([on, off], day).map((e) => e.id)).toEqual(["1"]);
  });
});

describe("colors", () => {
  it("maps category and event color", () => {
    expect(colorForCategory("work")).toBe("#3B82F6");
    expect(
      colorForEvent(ev({ id: "1", title: "X", category: "personal", color: null })),
    ).toBe("#8B5CF6");
    expect(
      colorForEvent(ev({ id: "1", title: "X", color: "#fff" })),
    ).toBe("#fff");
  });
});
