/** Bind a chat request to the view that sent it, and decide when drafts should persist. */

export type ChatViewBinding = {
  originConversationId: string | null;
  originViewId: number;
};

export function shouldPersistDraft(conversationId: string | null): conversationId is string {
  return Boolean(conversationId);
}

export function draftReadyToSave(args: {
  conversationId: string | null;
  ownerId: string | null;
  hydrated: boolean;
}): args is { conversationId: string; ownerId: string; hydrated: true } {
  return Boolean(
    args.hydrated && args.conversationId && args.ownerId === args.conversationId,
  );
}

export function draftPayload(input: string, answers: Record<string, string>) {
  return {
    composer: input,
    answers,
    clarification_answers: answers,
  };
}

export function lateReplyToastMessage(title: string | null | undefined): string {
  const label = (title || "").trim() || "another chat";
  return `Buddy replied in ${label}`;
}

export function chatResultBelongsToView(
  binding: ChatViewBinding,
  currentConversationId: string | null,
  currentViewId: number,
  responseConversationId: string,
): boolean {
  if (binding.originConversationId) {
    return currentConversationId === binding.originConversationId;
  }
  if (binding.originViewId !== currentViewId) return false;
  if (!currentConversationId) return true;
  return currentConversationId === responseConversationId;
}
