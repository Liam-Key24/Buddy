import { create } from "zustand";
import type {
  CalendarEvent,
  CalendarView,
  CreateEventInput,
  UpdateEventInput,
} from "@buddy/calendar/models";
import { CATEGORIES } from "@buddy/calendar/models";
import { mergeEvents } from "@buddy/calendar/services";
import { colorForEvent } from "@buddy/calendar/utils";
import { visibleRangeForView } from "@buddy/calendar/utils";
import {
  calendarCreateEvent,
  calendarDeleteEvent,
  calendarDuplicateEvent,
  calendarListEvents,
  calendarUpdateEvent,
} from "../lib/api";

export { colorForEvent };

interface CalendarState {
  events: CalendarEvent[];
  view: CalendarView;
  cursorDate: Date;
  loading: boolean;
  error: string | null;
  selectedEventId: string | null;
  formOpen: boolean;
  formMode: "create" | "edit";
  draftDefaults: CreateEventInput | null;
  searchQuery: string;
  enabledCategories: string[];
  deleteConfirmId: string | null;
  setView: (view: CalendarView) => void;
  setCursorDate: (date: Date) => void;
  selectEvent: (id: string | null) => void;
  setFormOpen: (
    open: boolean,
    mode?: "create" | "edit",
    draft?: CreateEventInput | null,
  ) => void;
  setSearchQuery: (q: string) => void;
  toggleCategory: (id: string) => void;
  setDeleteConfirmId: (id: string | null) => void;
  clearError: () => void;
  loadRange: () => Promise<void>;
  createEvent: (input: CreateEventInput) => Promise<void>;
  updateEvent: (id: string, input: UpdateEventInput) => Promise<void>;
  deleteEvent: (id: string) => Promise<void>;
  duplicateEvent: (id: string) => Promise<void>;
}

function defaultDraft(cursor: Date): CreateEventInput {
  const start = new Date(cursor);
  start.setMinutes(0, 0, 0);
  if (start.getTime() < Date.now()) {
    start.setHours(start.getHours() + 1);
  }
  const end = new Date(start.getTime() + 60 * 60 * 1000);
  return {
    title: "",
    start_time: start.getTime(),
    end_time: end.getTime(),
    all_day: false,
    category: "general",
    timezone: Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC",
    reminders: [{ minutes_before: 15, method: "popup" }],
  };
}

export const useCalendarStore = create<CalendarState>((set, get) => ({
  events: [],
  view: "month",
  cursorDate: new Date(),
  loading: false,
  error: null,
  selectedEventId: null,
  formOpen: false,
  formMode: "create",
  draftDefaults: null,
  searchQuery: "",
  enabledCategories: CATEGORIES.map((c) => c.id),
  deleteConfirmId: null,

  setView: (view) => {
    set({ view });
    void get().loadRange();
  },
  setCursorDate: (date) => {
    set({ cursorDate: date });
    void get().loadRange();
  },
  selectEvent: (id) => set({ selectedEventId: id }),
  setFormOpen: (open, mode = "create", draft = null) =>
    set({
      formOpen: open,
      formMode: mode,
      draftDefaults: draft ?? (open ? defaultDraft(get().cursorDate) : null),
    }),
  setSearchQuery: (q) => {
    set({ searchQuery: q });
    void get().loadRange();
  },
  toggleCategory: (id) => {
    const cur = get().enabledCategories;
    const next = cur.includes(id)
      ? cur.filter((c) => c !== id)
      : [...cur, id];
    set({ enabledCategories: next.length ? next : cur });
    void get().loadRange();
  },
  setDeleteConfirmId: (id) => set({ deleteConfirmId: id }),
  clearError: () => set({ error: null }),

  loadRange: async () => {
    const { view, cursorDate, searchQuery, enabledCategories } = get();
    const range = visibleRangeForView(view, cursorDate);
    set({ loading: true, error: null });
    try {
      const events = await calendarListEvents(
        range.start,
        range.end,
        searchQuery.trim() || undefined,
        enabledCategories,
      );
      set({ events, loading: false });
    } catch (e) {
      set({
        loading: false,
        error: e instanceof Error ? e.message : String(e),
      });
    }
  },

  createEvent: async (input) => {
    const optimistic: CalendarEvent = {
      id: `temp-${Date.now()}`,
      title: input.title,
      description: input.description ?? null,
      location: input.location ?? null,
      category: input.category ?? "general",
      color: input.color ?? null,
      start_time: input.start_time,
      end_time: input.end_time,
      all_day: input.all_day ?? false,
      timezone: input.timezone ?? "UTC",
      recurrence: input.recurrence ?? null,
      reminders: input.reminders ?? [],
      sync_status: "local",
      created_at: Date.now(),
      updated_at: Date.now(),
    };
    set((s) => ({ events: mergeEvents(s.events, [optimistic]), formOpen: false }));
    try {
      const created = await calendarCreateEvent(input);
      set((s) => ({
        events: mergeEvents(
          s.events.filter((e) => e.id !== optimistic.id),
          [created],
        ),
      }));
    } catch (e) {
      set((s) => ({
        events: s.events.filter((ev) => ev.id !== optimistic.id),
        error: e instanceof Error ? e.message : String(e),
      }));
      throw e;
    }
  },

  updateEvent: async (id, input) => {
    const prev = get().events;
    set((s) => ({
      events: s.events.map((e) =>
        e.id === id || e.occurrence_of === id
          ? {
              ...e,
              ...Object.fromEntries(
                Object.entries(input).filter(([, v]) => v !== undefined),
              ),
              updated_at: Date.now(),
            }
          : e,
      ),
      formOpen: false,
    }));
    try {
      const masterId = id.split("::")[0];
      const updated = await calendarUpdateEvent(masterId, input);
      await get().loadRange();
      set({ selectedEventId: updated.id });
    } catch (e) {
      set({
        events: prev,
        error: e instanceof Error ? e.message : String(e),
      });
      throw e;
    }
  },

  deleteEvent: async (id) => {
    const prev = get().events;
    const masterId = id.split("::")[0];
    set((s) => ({
      events: s.events.filter(
        (e) => e.id !== id && e.id !== masterId && e.occurrence_of !== masterId,
      ),
      selectedEventId: null,
      deleteConfirmId: null,
    }));
    try {
      await calendarDeleteEvent(masterId);
    } catch (e) {
      set({
        events: prev,
        error: e instanceof Error ? e.message : String(e),
      });
      throw e;
    }
  },

  duplicateEvent: async (id) => {
    try {
      const masterId = id.split("::")[0];
      const dup = await calendarDuplicateEvent(masterId);
      set((s) => ({
        events: mergeEvents(s.events, [dup]),
        selectedEventId: dup.id,
      }));
      await get().loadRange();
    } catch (e) {
      set({ error: e instanceof Error ? e.message : String(e) });
      throw e;
    }
  },
}));
