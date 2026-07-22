import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { EventDetailsDrawer } from "../../components/calendar/EventDetailsDrawer";
import type { CalendarEvent } from "@buddy/calendar/models";

const event: CalendarEvent = {
  id: "e1",
  title: "Standup",
  description: "Daily sync",
  location: "Zoom",
  category: "work",
  color: "#3B82F6",
  start_time: new Date(2026, 6, 19, 9).getTime(),
  end_time: new Date(2026, 6, 19, 9, 30).getTime(),
  all_day: false,
  timezone: "UTC",
  reminders: [{ minutes_before: 15, method: "popup" }],
  sync_status: "local",
  created_at: 0,
  updated_at: 0,
};

describe("EventDetailsDrawer", () => {
  it("renders empty when no event", () => {
    const { container } = render(
      <EventDetailsDrawer
        event={null}
        onClose={() => {}}
        onEdit={() => {}}
        onDuplicate={() => {}}
        onDelete={() => {}}
      />,
    );
    expect(container.firstChild).toBeNull();
  });

  it("renders event details", () => {
    render(
      <EventDetailsDrawer
        event={event}
        onClose={vi.fn()}
        onEdit={vi.fn()}
        onDuplicate={vi.fn()}
        onDelete={vi.fn()}
      />,
    );
    expect(screen.getByText("Standup")).toBeTruthy();
    expect(screen.getByText("Zoom")).toBeTruthy();
    expect(screen.getByText("Daily sync")).toBeTruthy();
  });
});
