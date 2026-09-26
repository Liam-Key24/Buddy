import { describe, expect, it } from "vitest";
import {
  chatResultBelongsToView,
  draftPayload,
  draftReadyToSave,
  lateReplyToastMessage,
  shouldPersistDraft,
} from "./chatSession";

describe("shouldPersistDraft", () => {
  it("saves clarification answers when the composer is empty", () => {
    expect(shouldPersistDraft("conv-1")).toBe(true);
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
    expect(shouldPersistDraft(conversationId)).toBe(true);
    const saved = draftPayload(input, answers);
    expect(saved.composer).toBe("");
    expect(saved.clarification_answers.grade).toBe("V3 indoors");
  });

  it("persists an empty composer and empty answers so clears survive reload", () => {
    expect(shouldPersistDraft("conv-1")).toBe(true);
    expect(draftPayload("", {})).toEqual({
      composer: "",
      answers: {},
      clarification_answers: {},
    });
  });

  it("does not persist when there is no conversation", () => {
    expect(shouldPersistDraft(null)).toBe(false);
  });

  it("does not save onto a chat that has not finished hydrating", () => {
    expect(
      draftReadyToSave({ conversationId: "b", ownerId: "a", hydrated: true }),
    ).toBe(false);
    expect(
      draftReadyToSave({ conversationId: "a", ownerId: "a", hydrated: false }),
    ).toBe(false);
    expect(
      draftReadyToSave({ conversationId: "a", ownerId: "a", hydrated: true }),
    ).toBe(true);
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

  it("keeps a late reply on the created conversation after a blank send", () => {
    const origin = { originConversationId: "created-from-new-chat", originViewId: 0 };
    expect(chatResultBelongsToView(origin, "opened-chat", 1, "created-from-new-chat")).toBe(
      false,
    );
    expect(lateReplyToastMessage("Climbing")).toBe("Buddy replied in Climbing");
    expect(lateReplyToastMessage("")).toBe("Buddy replied in another chat");
  });
});
