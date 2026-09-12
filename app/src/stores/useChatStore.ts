import { create } from "zustand";

export interface Message {
  id: string;
  role: "user" | "assistant";
  content: string;
  created_at: number;
}

export interface TraceStep {
  id: string;
  step: string;
  detail: string;
}

export interface AskOption {
  id: string;
  label: string;
  value: string;
}

export interface StructuredAsk {
  tool: string;
  question: string;
  field: string;
  ask_kind: "text" | "choice";
  options: AskOption[];
}

interface ChatState {
  messages: Message[];
  activeConversationId: string | null;
  isStreaming: boolean;
  streamingContent: string;
  traceSteps: TraceStep[];
  activeAsk: StructuredAsk | null;
  setMessages: (messages: Message[]) => void;
  addMessage: (message: Message) => void;
  beginSend: (text: string) => void;
  appendStreaming: (chunk: string) => void;
  finalizeStreaming: () => void;
  setActiveConversationId: (id: string | null) => void;
  setIsStreaming: (streaming: boolean) => void;
  clearStreaming: () => void;
  appendTrace: (step: string, detail: string) => void;
  clearTrace: () => void;
  setActiveAsk: (ask: StructuredAsk | null) => void;
}

export const useChatStore = create<ChatState>((set, get) => ({
  messages: [],
  activeConversationId: null,
  isStreaming: false,
  streamingContent: "",
  traceSteps: [],
  activeAsk: null,
  setMessages: (messages) => set({ messages }),
  addMessage: (message) =>
    set((state) => ({ messages: [...state.messages, message] })),
  beginSend: (text) =>
    set((state) => ({
      messages: [
        ...state.messages,
        {
          id: crypto.randomUUID(),
          role: "user",
          content: text,
          created_at: Date.now() / 1000,
        },
      ],
      streamingContent: "",
      isStreaming: true,
      traceSteps: [],
      activeAsk: null,
    })),
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
        traceSteps: [],
      }));
    } else {
      set({ isStreaming: false, traceSteps: [] });
    }
  },
  setActiveConversationId: (id) => set({ activeConversationId: id }),
  setIsStreaming: (streaming) => set({ isStreaming: streaming }),
  clearStreaming: () =>
    set({ streamingContent: "", isStreaming: false, traceSteps: [] }),
  appendTrace: (step, detail) =>
    set((state) => ({
      traceSteps: [
        ...state.traceSteps,
        { id: crypto.randomUUID(), step, detail },
      ],
    })),
  clearTrace: () => set({ traceSteps: [] }),
  setActiveAsk: (ask) => set({ activeAsk: ask }),
}));
