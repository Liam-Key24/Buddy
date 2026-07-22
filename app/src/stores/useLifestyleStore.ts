import { create } from "zustand";
import type {
  CreateDreamInput,
  DreamEntry,
  ScheduleBlock,
  ScheduleKind,
  UpdateDreamInput,
  WorkDayLog,
  WorkStats,
} from "@buddy/calendar/models";
import { visibleRangeForView } from "@buddy/calendar/utils";
import {
  dreamDelete,
  dreamList,
  dreamLog,
  dreamUpdate,
  lifestyleListBlocks,
  workGetDayLog,
  workGetStats,
  workLogSales,
  workSetHours,
} from "../lib/api";
import { useCalendarStore } from "./useCalendarStore";

interface LifestyleState {
  scheduleBlocks: ScheduleBlock[];
  showWork: boolean;
  showSleep: boolean;
  selectedBlockId: string | null;
  dreams: DreamEntry[];
  workStats: WorkStats | null;
  workDayLog: WorkDayLog | null;
  panelLoading: boolean;
  error: string | null;
  setShowWork: (v: boolean) => void;
  setShowSleep: (v: boolean) => void;
  selectBlock: (id: string | null) => void;
  clearError: () => void;
  loadBlocks: () => Promise<void>;
  loadDreamsForSelected: () => Promise<void>;
  loadWorkPanel: () => Promise<void>;
  addDream: (input: CreateDreamInput) => Promise<void>;
  updateDream: (id: string, input: UpdateDreamInput) => Promise<void>;
  removeDream: (id: string) => Promise<void>;
  saveSales: (amount: number) => Promise<void>;
  saveEndTime: (endMs: number) => Promise<void>;
}

function selectedBlock(state: LifestyleState): ScheduleBlock | null {
  return state.scheduleBlocks.find((b) => b.id === state.selectedBlockId) ?? null;
}

export const useLifestyleStore = create<LifestyleState>((set, get) => ({
  scheduleBlocks: [],
  showWork: true,
  showSleep: true,
  selectedBlockId: null,
  dreams: [],
  workStats: null,
  workDayLog: null,
  panelLoading: false,
  error: null,

  setShowWork: (v) => set({ showWork: v }),
  setShowSleep: (v) => set({ showSleep: v }),
  selectBlock: (id) => {
    set({ selectedBlockId: id });
    if (id) {
      useCalendarStore.getState().selectEvent(null);
      const block = get().scheduleBlocks.find((b) => b.id === id);
      if (block?.kind === "sleep") void get().loadDreamsForSelected();
      if (block?.kind === "work") void get().loadWorkPanel();
    }
  },
  clearError: () => set({ error: null }),

  loadBlocks: async () => {
    const { view, cursorDate } = useCalendarStore.getState();
    const range = visibleRangeForView(view, cursorDate);
    try {
      const blocks = await lifestyleListBlocks(range.start, range.end);
      set({ scheduleBlocks: blocks, error: null });
    } catch (e) {
      set({ error: e instanceof Error ? e.message : String(e) });
    }
  },

  loadDreamsForSelected: async () => {
    const block = selectedBlock(get());
    if (!block || block.kind !== "sleep") {
      set({ dreams: [] });
      return;
    }
    set({ panelLoading: true });
    try {
      const dreams = await dreamList(block.anchor_date);
      set({ dreams, panelLoading: false, error: null });
    } catch (e) {
      set({
        panelLoading: false,
        error: e instanceof Error ? e.message : String(e),
      });
    }
  },

  loadWorkPanel: async () => {
    const block = selectedBlock(get());
    if (!block || block.kind !== "work") {
      set({ workStats: null, workDayLog: null });
      return;
    }
    set({ panelLoading: true });
    try {
      const [stats, log] = await Promise.all([
        workGetStats(),
        workGetDayLog(block.anchor_date),
      ]);
      set({
        workStats: stats,
        workDayLog: log,
        panelLoading: false,
        error: null,
      });
    } catch (e) {
      set({
        panelLoading: false,
        error: e instanceof Error ? e.message : String(e),
      });
    }
  },

  addDream: async (input) => {
    const block = selectedBlock(get());
    await dreamLog({
      ...input,
      sleep_date: input.sleep_date ?? block?.anchor_date ?? null,
    });
    await get().loadDreamsForSelected();
  },

  updateDream: async (id, input) => {
    await dreamUpdate(id, input);
    await get().loadDreamsForSelected();
  },

  removeDream: async (id) => {
    await dreamDelete(id);
    await get().loadDreamsForSelected();
  },

  saveSales: async (amount) => {
    const block = selectedBlock(get());
    await workLogSales(amount, block?.anchor_date ?? null, "GBP");
    await get().loadWorkPanel();
  },

  saveEndTime: async (endMs) => {
    const block = selectedBlock(get());
    await workSetHours(block?.anchor_date ?? null, null, endMs);
    await get().loadWorkPanel();
  },
}));

export function visibleScheduleBlocks(
  blocks: ScheduleBlock[],
  showWork: boolean,
  showSleep: boolean,
): ScheduleBlock[] {
  return blocks.filter((b) => {
    if (b.kind === "work") return showWork;
    if (b.kind === "sleep") return showSleep;
    return false;
  });
}

export function isScheduleKind(kind: string): kind is ScheduleKind {
  return kind === "work" || kind === "sleep";
}
