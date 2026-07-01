import { create } from "zustand";

export interface Message {
  id: string;
  role: "user" | "assistant";
  content: string;
  created_at: number;
}

interface ChatState {
  messages: Message[];
  activeConversationId: string | null;
  isStreaming: boolean;
  streamingContent: string;
  setMessages: (messages: Message[]) => void;
  addMessage: (message: Message) => void;
  appendStreaming: (chunk: string) => void;
  finalizeStreaming: () => void;
  setActiveConversationId: (id: string | null) => void;
  setIsStreaming: (streaming: boolean) => void;
  clearStreaming: () => void;
}

export const useChatStore = create<ChatState>((set, get) => ({
  messages: [],
  activeConversationId: null,
  isStreaming: false,
  streamingContent: "",
  setMessages: (messages) => set({ messages }),
  addMessage: (message) =>
    set((state) => ({ messages: [...state.messages, message] })),
  appendStreaming: (chunk) =>
    set((state) => ({ streamingContent: state.streamingContent + chunk })),
  finalizeStreaming: () => {
    const { streamingContent } = get();
    if (streamingContent) {
      set((state) => ({
        messages: [
          ...state.messages,
          {
            id: crypto.randomUUID(),
            role: "assistant",
            content: streamingContent,
            created_at: Date.now() / 1000,
          },
        ],
        streamingContent: "",
        isStreaming: false,
      }));
    } else {
      set({ isStreaming: false });
    }
  },
  setActiveConversationId: (id) => set({ activeConversationId: id }),
  setIsStreaming: (streaming) => set({ isStreaming: streaming }),
  clearStreaming: () => set({ streamingContent: "", isStreaming: false }),
}));
