import { create } from "zustand";

export type ServiceStatus = "online" | "offline" | "checking";

interface AppState {
  mlxStatus: ServiceStatus;
  brainStatus: ServiceStatus;
  currentPage: "dashboard" | "chat" | "settings";
  sidebarCollapsed: boolean;
  setMlxStatus: (status: ServiceStatus) => void;
  setBrainStatus: (status: ServiceStatus) => void;
  setCurrentPage: (page: "dashboard" | "chat" | "settings") => void;
  setSidebarCollapsed: (collapsed: boolean) => void;
  toggleSidebar: () => void;
}

export const useAppStore = create<AppState>((set) => ({
  mlxStatus: "checking",
  brainStatus: "checking",
  currentPage: "dashboard",
  sidebarCollapsed: false,
  setMlxStatus: (status) => set({ mlxStatus: status }),
  setBrainStatus: (status) => set({ brainStatus: status }),
  setCurrentPage: (page) => set({ currentPage: page }),
  setSidebarCollapsed: (collapsed) => set({ sidebarCollapsed: collapsed }),
  toggleSidebar: () => set((s) => ({ sidebarCollapsed: !s.sidebarCollapsed })),
}));
