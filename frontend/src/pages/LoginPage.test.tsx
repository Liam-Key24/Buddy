import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { LoginPage } from "./LoginPage";

describe("LoginPage", () => {
  it("submits username and password", async () => {
    const onSubmit = vi.fn().mockResolvedValue({ id: "1", username: "liam" });
    const onLoggedIn = vi.fn();
    render(<LoginPage onLoggedIn={onLoggedIn} onSubmit={onSubmit} />);
    fireEvent.change(screen.getByLabelText("Username"), { target: { value: "liam" } });
    fireEvent.change(screen.getByLabelText("Password"), { target: { value: "secret" } });
    fireEvent.click(screen.getByRole("button", { name: "Sign in" }));
    await waitFor(() => expect(onSubmit).toHaveBeenCalledWith("liam", "secret"));
    expect(onLoggedIn).toHaveBeenCalledWith({ id: "1", username: "liam" });
  });
});
