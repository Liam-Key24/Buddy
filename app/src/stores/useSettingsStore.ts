import { create } from "zustand";

interface SettingsState {
  mlxUrl: string;
  brainUrl: string;
  modelName: string;
  logLevel: string;
  autoStartMlx: boolean;
  setSettings: (settings: Partial<Omit<SettingsState, "setSettings">>) => void;
}

export const useSettingsStore = create<SettingsState>((set) => ({
  mlxUrl: "http://127.0.0.1:8001",
  brainUrl: "http://127.0.0.1:8002",
  modelName: "mlx-community/Llama-3.2-3B-Instruct-4bit",
  logLevel: "info",
  autoStartMlx: false,
  setSettings: (settings) => set(settings),
}));
