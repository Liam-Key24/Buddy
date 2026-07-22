import { create } from "zustand";

export type ServiceStatus = "online" | "offline" | "checking";

export type AppPage =
  | "dashboard"
  | "chat"
  | "spark"
  | "settings"
  | "code"
  | "calendar";

interface AppState {
  mlxStatus: ServiceStatus;
  brainStatus: ServiceStatus;
  currentPage: AppPage;
  sidebarCollapsed: boolean;
  pendingChatMessage: string | null;
  setMlxStatus: (status: ServiceStatus) => void;
  setBrainStatus: (status: ServiceStatus) => void;
  setCurrentPage: (page: AppPage) => void;
  setSidebarCollapsed: (collapsed: boolean) => void;
  toggleSidebar: () => void;
  setPendingChatMessage: (message: string | null) => void;
}

export const useAppStore = create<AppState>((set) => ({
  mlxStatus: "checking",
  brainStatus: "checking",
  currentPage: "dashboard",
  sidebarCollapsed: false,
  pendingChatMessage: null,
  setMlxStatus: (status) => set({ mlxStatus: status }),
  setBrainStatus: (status) => set({ brainStatus: status }),
  setCurrentPage: (page) => set({ currentPage: page }),
  setSidebarCollapsed: (collapsed) => set({ sidebarCollapsed: collapsed }),
  toggleSidebar: () => set((s) => ({ sidebarCollapsed: !s.sidebarCollapsed })),
  setPendingChatMessage: (message) => set({ pendingChatMessage: message }),
}));
