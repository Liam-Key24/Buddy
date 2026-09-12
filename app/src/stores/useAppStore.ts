import { create } from "zustand";

export type ServiceStatus = "online" | "offline" | "checking";

export type ChatMode = "talk" | "tool";

export type AppPage =
  | "dashboard"
  | "chat"
  | "spark"
  | "settings"
  | "code"
  | "calendar"
  | "documents"
  | "fitness"
  | "money"
  | "study"
  | "todo"
  | "socials";

const CHAT_MODE_KEY = "buddy.chatMode";

function loadChatMode(): ChatMode {
  try {
    const value = localStorage.getItem(CHAT_MODE_KEY);
    if (value === "talk" || value === "tool") return value;
  } catch {
    /* ignore */
  }
  return "tool";
}

interface AppState {
  mlxStatus: ServiceStatus;
  brainStatus: ServiceStatus;
  currentPage: AppPage;
  sidebarCollapsed: boolean;
  pendingChatMessage: string | null;
  chatMode: ChatMode;
  pendingWorkspaceDocId: string | null;
  setMlxStatus: (status: ServiceStatus) => void;
  setBrainStatus: (status: ServiceStatus) => void;
  setCurrentPage: (page: AppPage) => void;
  setSidebarCollapsed: (collapsed: boolean) => void;
  toggleSidebar: () => void;
  setPendingChatMessage: (message: string | null) => void;
  setChatMode: (mode: ChatMode) => void;
  setPendingWorkspaceDocId: (id: string | null) => void;
}

export const useAppStore = create<AppState>((set) => ({
  mlxStatus: "checking",
  brainStatus: "checking",
  currentPage: "dashboard",
  sidebarCollapsed: false,
  pendingChatMessage: null,
  chatMode: loadChatMode(),
  pendingWorkspaceDocId: null,
  setMlxStatus: (status) => set({ mlxStatus: status }),
  setBrainStatus: (status) => set({ brainStatus: status }),
  setCurrentPage: (page) => set({ currentPage: page }),
  setSidebarCollapsed: (collapsed) => set({ sidebarCollapsed: collapsed }),
  toggleSidebar: () => set((s) => ({ sidebarCollapsed: !s.sidebarCollapsed })),
  setPendingChatMessage: (message) => set({ pendingChatMessage: message }),
  setChatMode: (mode) => {
    try {
      localStorage.setItem(CHAT_MODE_KEY, mode);
    } catch {
      /* ignore */
    }
    set({ chatMode: mode });
  },
  setPendingWorkspaceDocId: (id) => set({ pendingWorkspaceDocId: id }),
}));
