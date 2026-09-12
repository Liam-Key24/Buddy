import { create } from "zustand";

export type ServiceStatus = "online" | "offline" | "checking";

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

interface AppState {
  mlxStatus: ServiceStatus;
  brainStatus: ServiceStatus;
  runtimeStarting: boolean;
  runtimeError: string | null;
  currentPage: AppPage;
  sidebarCollapsed: boolean;
  pendingChatMessage: string | null;
  pendingWorkspaceDocId: string | null;
  setMlxStatus: (status: ServiceStatus) => void;
  setBrainStatus: (status: ServiceStatus) => void;
  setRuntimeStarting: (starting: boolean) => void;
  setRuntimeError: (error: string | null) => void;
  setCurrentPage: (page: AppPage) => void;
  setSidebarCollapsed: (collapsed: boolean) => void;
  toggleSidebar: () => void;
  setPendingChatMessage: (message: string | null) => void;
  setPendingWorkspaceDocId: (id: string | null) => void;
}

export const useAppStore = create<AppState>((set) => ({
  mlxStatus: "checking",
  brainStatus: "checking",
  runtimeStarting: false,
  runtimeError: null,
  currentPage: "dashboard",
  sidebarCollapsed: false,
  pendingChatMessage: null,
  pendingWorkspaceDocId: null,
  setMlxStatus: (status) => set({ mlxStatus: status }),
  setBrainStatus: (status) => set({ brainStatus: status }),
  setRuntimeStarting: (runtimeStarting) => set({ runtimeStarting }),
  setRuntimeError: (runtimeError) => set({ runtimeError }),
  setCurrentPage: (page) => set({ currentPage: page }),
  setSidebarCollapsed: (collapsed) => set({ sidebarCollapsed: collapsed }),
  toggleSidebar: () => set((s) => ({ sidebarCollapsed: !s.sidebarCollapsed })),
  setPendingChatMessage: (message) => set({ pendingChatMessage: message }),
  setPendingWorkspaceDocId: (id) => set({ pendingWorkspaceDocId: id }),
}));
