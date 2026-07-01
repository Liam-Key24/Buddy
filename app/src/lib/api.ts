import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import { useChatStore } from "../stores/useChatStore";
import { useConversationStore } from "../stores/useConversationStore";

export interface ServiceStatusResponse {
  mlx: boolean;
  brain: boolean;
}

export async function fetchServiceStatus(): Promise<ServiceStatusResponse> {
  return invoke("get_service_status");
}

export async function startBrain(): Promise<void> {
  return invoke("start_brain");
}

export async function loadConversations() {
  const conversations = await invoke<
    Array<{
      id: string;
      title: string;
      created_at: number;
      updated_at: number;
    }>
  >("list_conversations");
  useConversationStore.getState().setConversations(conversations);
  return conversations;
}

export async function createConversation(title?: string) {
  const conv = await invoke<{
    id: string;
    title: string;
    created_at: number;
    updated_at: number;
  }>("create_conversation", { title });
  useConversationStore.getState().addConversation(conv);
  return conv;
}

export async function deleteConversation(id: string) {
  try {
    await invoke("delete_conversation", { id });
  } finally {
    await loadConversations();
  }
}

export async function loadMessages(
  conversationId: string,
  options?: { force?: boolean },
) {
  if (!options?.force && useChatStore.getState().isStreaming) {
    return;
  }

  const messages = await invoke<
    Array<{
      id: string;
      conversation_id: string;
      role: string;
      content: string;
      created_at: number;
    }>
  >("get_messages", { conversationId });
  useChatStore.getState().setMessages(
    messages.map((m) => ({
      id: m.id,
      role: m.role as "user" | "assistant",
      content: m.content,
      created_at: m.created_at,
    })),
  );
}

export async function sendMessage(
  conversationId: string,
  text: string,
  options?: { skipOptimistic?: boolean },
) {
  const chat = useChatStore.getState();
  if (!options?.skipOptimistic) {
    chat.beginSend(text);
  }

  const unlistenChunk = await listen<string>("chat-chunk", (event) => {
    useChatStore.getState().appendStreaming(event.payload);
  });
  const unlistenDone = await listen("chat-done", async () => {
    await loadMessages(conversationId, { force: true });
    useChatStore.getState().clearStreaming();
    unlistenChunk();
    unlistenDone();
  });

  try {
    await invoke("send_message", { conversationId, text });
    await loadConversations();
  } catch (error) {
    chat.clearStreaming();
    unlistenChunk();
    unlistenDone();
    throw error;
  }
}

export async function loadSettings() {
  const settings = await invoke<{
    mlx_url: string;
    brain_url: string;
    model_name: string;
    log_level: string;
    auto_start_mlx: boolean;
  }>("get_settings");
  return settings;
}

export async function saveSetting(key: string, value: string) {
  await invoke("set_setting", { key, value });
}
