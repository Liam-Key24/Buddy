import { create } from "zustand";

interface SettingsState {
  mlxUrl: string;
  brainUrl: string;
  modelName: string;
  logLevel: string;
  autoStartMlx: boolean;
  modelNameChat: string;
  modelNameCode: string;
  llmProfileRouter: string;
  codexModel: string;
  codexWorkspace: string;
  codeAgentBackend: string;
  codeModel: string;
  cursorPath: string;
  codexPath: string;
  emailSignature: string;
  emailGreeting: string;
  emailBodyTemplate: string;
  fsExcludedPaths: string[];
  calendarNotificationsEnabled: boolean;
  calendarDefaultTimezone: string;
  calendarDefaultRemindersJson: string;
  setSettings: (settings: Partial<Omit<SettingsState, "setSettings">>) => void;
}

export const useSettingsStore = create<SettingsState>((set) => ({
  mlxUrl: "http://127.0.0.1:8001",
  brainUrl: "http://127.0.0.1:8002",
  modelName: "mlx-community/Llama-3.2-3B-Instruct-4bit",
  logLevel: "info",
  autoStartMlx: false,
  modelNameChat: "mlx-community/Llama-3.2-3B-Instruct-4bit",
  modelNameCode: "mlx-community/Llama-3.2-3B-Instruct-4bit",
  llmProfileRouter: "mlx-community/Llama-3.2-3B-Instruct-4bit",
  codexModel: "gpt-5.5",
  codexWorkspace: "",
  codeAgentBackend: "cursor",
  codeModel: "auto",
  cursorPath: "",
  codexPath: "",
  emailSignature: "",
  emailGreeting: "Hi,",
  emailBodyTemplate: "{greeting}\n\n{body}\n\n{signature}",
  fsExcludedPaths: [],
  calendarNotificationsEnabled: true,
  calendarDefaultTimezone:
    Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC",
  calendarDefaultRemindersJson: JSON.stringify([
    { minutes_before: 15, method: "popup" },
  ]),
  setSettings: (settings) => set(settings),
}));
