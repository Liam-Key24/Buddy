import { create } from "zustand";

export type FocusMode = "planning" | "asking" | "debugging" | "focused";

export interface CodeMessage {
  id: string;
  role: "user" | "assistant";
  content: string;
  created_at: number;
}

interface CodeAgentState {
  messages: CodeMessage[];
  activeConversationId: string | null;
  isStreaming: boolean;
  streamingContent: string;
  focus: FocusMode;
  attachments: string[];
  previewUrl: string | null;
  workspacePath: string | null;
  setMessages: (messages: CodeMessage[]) => void;
  beginSend: (text: string) => void;
  appendStreaming: (chunk: string) => void;
  clearStreaming: () => void;
  setActiveConversationId: (id: string | null) => void;
  setFocus: (focus: FocusMode) => void;
  addAttachment: (path: string) => void;
  removeAttachment: (path: string) => void;
  clearAttachments: () => void;
  setPreviewUrl: (url: string | null) => void;
  setWorkspacePath: (path: string | null) => void;
}

export const useCodeAgentStore = create<CodeAgentState>((set) => ({
  messages: [],
  activeConversationId: null,
  isStreaming: false,
  streamingContent: "",
  focus: "planning",
  attachments: [],
  previewUrl: null,
  workspacePath: null,
  setMessages: (messages) => set({ messages }),
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
    })),
  appendStreaming: (chunk) =>
    set((state) => ({ streamingContent: state.streamingContent + chunk })),
  clearStreaming: () => set({ streamingContent: "", isStreaming: false }),
  setActiveConversationId: (id) => set({ activeConversationId: id }),
  setFocus: (focus) => set({ focus }),
  addAttachment: (path) =>
    set((state) =>
      state.attachments.includes(path)
        ? state
        : { attachments: [...state.attachments, path] },
    ),
  removeAttachment: (path) =>
    set((state) => ({
      attachments: state.attachments.filter((p) => p !== path),
    })),
  clearAttachments: () => set({ attachments: [] }),
  setPreviewUrl: (url) => set({ previewUrl: url }),
  setWorkspacePath: (path) => set({ workspacePath: path }),
}));

export const FOCUS_LABELS: Record<FocusMode, string> = {
  planning: "Planning",
  asking: "Asking",
  debugging: "Debugging",
  focused: "Focused",
};
