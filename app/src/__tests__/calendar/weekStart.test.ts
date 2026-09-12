import { describe, expect, it } from "vitest";
import { monthGridDays, startOfWeek, WEEKDAY_SHORT } from "@buddy/calendar/utils";

describe("Monday-start week", () => {
  it("startOfWeek lands on Monday", () => {
    const saturday = new Date(2026, 7, 8); // Aug 8 2026 is Saturday
    const start = startOfWeek(saturday);
    expect(start.getDay()).toBe(1);
    expect(start.getDate()).toBe(3);
  });

  it("month grid starts on Monday", () => {
    const days = monthGridDays(new Date(2026, 7, 1));
    expect(days[0].getDay()).toBe(1);
    expect(WEEKDAY_SHORT[0]).toBe("Mon");
    expect(WEEKDAY_SHORT[6]).toBe("Sun");
  });
});
