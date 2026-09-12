import { create } from "zustand";

interface LifeChatContextState {
  summary: string;
  setContext: (summary: string) => void;
  clearContext: () => void;
}

export const useLifeChatContext = create<LifeChatContextState>((set) => ({
  summary: "",
  setContext: (summary) => set({ summary }),
  clearContext: () => set({ summary: "" }),
}));
