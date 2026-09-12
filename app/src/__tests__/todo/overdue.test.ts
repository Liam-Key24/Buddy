import { describe, expect, it } from "vitest";
import { isOverdue } from "../../stores/useTodoStore";
import type { Todo } from "../../lib/lifeApi";

function todo(partial: Partial<Todo> & Pick<Todo, "deadline" | "status">): Todo {
  return {
    id: "1",
    title: "x",
    description: null,
    priority: "medium",
    category: "general",
    notes: null,
    recurrence: "none",
    completed_at: null,
    created_at: 0,
    updated_at: 0,
    ...partial,
  };
}

describe("todo overdue", () => {
  it("flags open todos past deadline", () => {
    expect(isOverdue(todo({ deadline: "2026-08-01", status: "in_progress" }), "2026-08-08")).toBe(
      true,
    );
  });

  it("ignores completed and undated", () => {
    expect(isOverdue(todo({ deadline: "2026-08-01", status: "completed" }), "2026-08-08")).toBe(
      false,
    );
    expect(isOverdue(todo({ deadline: null, status: "not_started" }), "2026-08-08")).toBe(false);
  });
});
