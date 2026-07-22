import { create } from "zustand";
import type { ReminderDelivery } from "@buddy/calendar/models";
import {
  calendarDismissReminder,
  calendarListNotifications,
  calendarNotificationCount,
  calendarSnoozeReminder,
} from "../lib/api";

interface CalendarNotificationState {
  notifications: ReminderDelivery[];
  count: number;
  panelOpen: boolean;
  setPanelOpen: (open: boolean) => void;
  pushReminder: (delivery: ReminderDelivery) => void;
  setCount: (count: number) => void;
  refresh: () => Promise<void>;
  snooze: (id: string, minutes: number) => Promise<void>;
  dismiss: (id: string) => Promise<void>;
}

export const useCalendarNotificationStore = create<CalendarNotificationState>(
  (set, get) => ({
    notifications: [],
    count: 0,
    panelOpen: false,
    setPanelOpen: (open) => set({ panelOpen: open }),
    pushReminder: (delivery) =>
      set((s) => ({
        notifications: [
          delivery,
          ...s.notifications.filter((n) => n.id !== delivery.id),
        ],
        count: s.count + 1,
        panelOpen: true,
      })),
    setCount: (count) => set({ count }),
    refresh: async () => {
      try {
        const [notifications, count] = await Promise.all([
          calendarListNotifications(),
          calendarNotificationCount(),
        ]);
        set({ notifications, count });
      } catch (e) {
        console.error(e);
      }
    },
    snooze: async (id, minutes) => {
      await calendarSnoozeReminder(id, minutes);
      set((s) => ({
        notifications: s.notifications.map((n) =>
          n.id === id
            ? {
                ...n,
                status: "snoozed",
                snoozed_until: Date.now() + minutes * 60_000,
              }
            : n,
        ),
      }));
      await get().refresh();
    },
    dismiss: async (id) => {
      await calendarDismissReminder(id);
      set((s) => ({
        notifications: s.notifications.filter((n) => n.id !== id),
        count: Math.max(0, s.count - 1),
      }));
      await get().refresh();
    },
  }),
);
