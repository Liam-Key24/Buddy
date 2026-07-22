import type { RefObject } from "react";
import { BellSlash, Clock } from "@phosphor-icons/react";
import { SNOOZE_OPTIONS_MINUTES } from "@buddy/calendar/notifications";
import { useCalendarNotificationStore } from "../../stores/useCalendarNotificationStore";
import { useCalendarStore } from "../../stores/useCalendarStore";

export function NotificationPanel({
  panelRef,
}: {
  panelRef?: RefObject<HTMLDivElement | null>;
} = {}) {
  const { notifications, panelOpen, setPanelOpen, snooze, dismiss } =
    useCalendarNotificationStore();
  const selectEvent = useCalendarStore((s) => s.selectEvent);

  if (!panelOpen) return null;

  return (
    <div
      ref={panelRef}
      className="absolute right-0 top-12 z-30 w-80 rounded-2xl border border-zinc-800 bg-zinc-900 p-3 shadow-2xl shadow-black/40"
    >
      <div className="mb-2 flex items-center justify-between px-1">
        <h4 className="text-sm font-medium text-zinc-200">Reminders</h4>
        <button
          type="button"
          onClick={() => setPanelOpen(false)}
          className="text-xs text-zinc-500 hover:text-zinc-300"
        >
          Close
        </button>
      </div>
      {notifications.length === 0 ? (
        <div className="flex flex-col items-center gap-2 py-8 text-zinc-600">
          <BellSlash size={28} />
          <p className="text-xs">No active reminders</p>
        </div>
      ) : (
        <ul className="max-h-80 space-y-2 overflow-y-auto">
          {notifications.map((n) => (
            <li
              key={n.id}
              className="rounded-xl border border-zinc-800 bg-zinc-950/50 p-3"
            >
              <button
                type="button"
                onClick={() => {
                  selectEvent(n.event_id);
                  setPanelOpen(false);
                }}
                className="w-full text-left"
              >
                <div className="text-sm font-medium text-zinc-200">
                  {n.event_title}
                </div>
                <div className="mt-0.5 flex items-center gap-1 text-[11px] text-zinc-500">
                  <Clock size={12} />
                  {n.reminder_minutes} min before · {n.status}
                </div>
              </button>
              <div className="mt-2 flex flex-wrap gap-1">
                {SNOOZE_OPTIONS_MINUTES.map((m) => (
                  <button
                    key={m}
                    type="button"
                    onClick={() => snooze(n.id, m)}
                    className="rounded-lg border border-zinc-700 px-2 py-0.5 text-[10px] text-zinc-400 hover:bg-zinc-800"
                  >
                    {m}m
                  </button>
                ))}
                <button
                  type="button"
                  onClick={() => dismiss(n.id)}
                  className="rounded-lg border border-zinc-700 px-2 py-0.5 text-[10px] text-zinc-400 hover:border-rose-500/40 hover:text-rose-400"
                >
                  Dismiss
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
