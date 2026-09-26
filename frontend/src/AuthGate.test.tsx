import { render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({
  fetchMe: vi.fn(),
  login: vi.fn(),
}));

vi.mock("./api", () => api);

import { AuthGate } from "./AuthGate";

describe("AuthGate", () => {
  it("shows login when the session is missing", async () => {
    api.fetchMe.mockResolvedValue(null);
    render(
      <AuthGate>
        <div>shell</div>
      </AuthGate>,
    );
    await waitFor(() => expect(screen.getByRole("button", { name: "Sign in" })).toBeInTheDocument());
    expect(screen.queryByText("shell")).toBeNull();
  });

  it("shows the app when /me succeeds", async () => {
    api.fetchMe.mockResolvedValue({ id: "1", username: "liam" });
    render(
      <AuthGate>
        <div>shell</div>
      </AuthGate>,
    );
    await waitFor(() => expect(screen.getByText("shell")).toBeInTheDocument());
  });
});
