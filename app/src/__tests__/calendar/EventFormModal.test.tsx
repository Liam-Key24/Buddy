import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, fireEvent } from "@testing-library/react";
import { EventFormModal } from "../../components/calendar/EventFormModal";

afterEach(() => cleanup());

describe("EventFormModal", () => {
  it("requires a title", async () => {
    const onSubmit = vi.fn();
    const start = Date.now();
    render(
      <EventFormModal
        mode="create"
        initial={{
          title: "",
          start_time: start,
          end_time: start + 3600000,
          timezone: "UTC",
        }}
        onClose={() => {}}
        onSubmit={onSubmit}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: /^save$/i }));
    expect(await screen.findByText(/title is required/i)).toBeTruthy();
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("rejects end before start", async () => {
    const onSubmit = vi.fn();
    const start = Date.now() + 7200000;
    const end = Date.now() + 3600000;
    render(
      <EventFormModal
        mode="create"
        initial={{
          title: "Meet",
          start_time: start,
          end_time: end,
          timezone: "UTC",
        }}
        onClose={() => {}}
        onSubmit={onSubmit}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: /^save$/i }));
    expect(await screen.findByText(/end must be after start/i)).toBeTruthy();
    expect(onSubmit).not.toHaveBeenCalled();
  });
});
