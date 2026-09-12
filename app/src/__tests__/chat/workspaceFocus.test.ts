import { describe, expect, it } from "vitest";
import { pickWorkspacePage } from "../../lib/workspaceFocus";

describe("pickWorkspacePage", () => {
  it("opens documents for a spark-to-doc-to-week dump", () => {
    expect(
      pickWorkspacePage(["spark", "documents", "calendar"], "chat"),
    ).toBe("documents");
  });

  it("stays put when already on the primary workspace", () => {
    expect(pickWorkspacePage(["fitness"], "fitness")).toBeNull();
    expect(pickWorkspacePage(["documents", "calendar"], "documents")).toBeNull();
  });

  it("leaves a lower-priority page for calendar writes", () => {
    expect(pickWorkspacePage(["fitness", "calendar"], "fitness")).toBe(
      "calendar",
    );
  });

  it("opens fitness from main chat after a food dump", () => {
    expect(pickWorkspacePage(["fitness"], "chat")).toBe("fitness");
  });
});
