import { create } from "zustand";

export type ServiceStatus = "online" | "offline" | "checking";

interface AppState {
  mlxStatus: ServiceStatus;
  brainStatus: ServiceStatus;
  currentPage: "chat" | "settings";
  setMlxStatus: (status: ServiceStatus) => void;
  setBrainStatus: (status: ServiceStatus) => void;
  setCurrentPage: (page: "chat" | "settings") => void;
}

export const useAppStore = create<AppState>((set) => ({
  mlxStatus: "checking",
  brainStatus: "checking",
  currentPage: "chat",
  setMlxStatus: (status) => set({ mlxStatus: status }),
  setBrainStatus: (status) => set({ brainStatus: status }),
  setCurrentPage: (page) => set({ currentPage: page }),
}));
