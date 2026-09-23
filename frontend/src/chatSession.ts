/** Bind a chat request to the view that sent it, and decide when drafts should persist. */

export type ChatViewBinding = {
  originConversationId: string | null;
  originViewId: number;
};

export function shouldPersistDraft(
  conversationId: string | null,
  input: string,
  answers: Record<string, string>,
): conversationId is string {
  if (!conversationId) return false;
  if (input.length > 0) return true;
  return Object.values(answers).some((value) => String(value ?? "").trim().length > 0);
}

export function draftPayload(input: string, answers: Record<string, string>) {
  return {
    composer: input,
    answers,
    clarification_answers: answers,
  };
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
