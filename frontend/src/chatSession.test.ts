import { describe, expect, it } from "vitest";
import {
  chatResultBelongsToView,
  draftPayload,
  shouldPersistDraft,
} from "./chatSession";

describe("shouldPersistDraft", () => {
  it("saves clarification answers when the composer is empty", () => {
    expect(shouldPersistDraft("conv-1", "", { grade: "V3 indoors" })).toBe(true);
    expect(draftPayload("", { grade: "V3 indoors" })).toEqual({
      composer: "",
      answers: { grade: "V3 indoors" },
      clarification_answers: { grade: "V3 indoors" },
    });
  });

  it("keeps answer-only drafts across a reload with an empty composer", () => {
    const conversationId = "conv-1";
    const input = "";
    const answers = { grade: "V3 indoors" };
    expect(shouldPersistDraft(conversationId, input, answers)).toBe(true);
    const saved = draftPayload(input, answers);
    const restoredAnswers = saved.clarification_answers;
    expect(saved.composer).toBe("");
    expect(restoredAnswers.grade).toBe("V3 indoors");
  });

  it("does not persist when there is no conversation", () => {
    expect(shouldPersistDraft(null, "", { grade: "V3" })).toBe(false);
  });
});

describe("chatResultBelongsToView", () => {
  it("keeps a late new-chat response out of a chat opened before it arrived", () => {
    const origin = { originConversationId: null as string | null, originViewId: 0 };
    expect(
      chatResultBelongsToView(origin, "opened-chat", 1, "created-from-new-chat"),
    ).toBe(false);
  });

  it("applies a new-chat response if the originating blank view is still open", () => {
    const origin = { originConversationId: null as string | null, originViewId: 0 };
    expect(chatResultBelongsToView(origin, null, 0, "created-from-new-chat")).toBe(true);
    expect(
      chatResultBelongsToView(origin, "created-from-new-chat", 0, "created-from-new-chat"),
    ).toBe(true);
  });

  it("does not apply a late response after switching away from an existing chat", () => {
    const origin = { originConversationId: "chat-a", originViewId: 2 };
    expect(chatResultBelongsToView(origin, "chat-b", 3, "chat-a")).toBe(false);
    expect(chatResultBelongsToView(origin, "chat-a", 2, "chat-a")).toBe(true);
  });
});
