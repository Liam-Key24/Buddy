import {
  Bell,
  CaretLeft,
  CaretRight,
} from "@phosphor-icons/react";
import type { CalendarView } from "@buddy/calendar/models";
import { formatMonthYear } from "@buddy/calendar/utils";

const VIEWS: { id: CalendarView; label: string }[] = [
  { id: "day", label: "Day" },
  { id: "week", label: "Week" },
  { id: "month", label: "Month" },
  { id: "agenda", label: "Agenda" },
];

export function CalendarHeader({
  cursorDate,
  view,
  notificationCount,
  onPrev,
  onNext,
  onToday,
  onViewChange,
  onToggleNotifications,
}: {
  cursorDate: Date;
  view: CalendarView;
  notificationCount: number;
  onPrev: () => void;
  onNext: () => void;
  onToday: () => void;
  onViewChange: (v: CalendarView) => void;
  onToggleNotifications: () => void;
}) {
  const title =
    view === "day"
      ? cursorDate.toLocaleDateString(undefined, {
          weekday: "long",
          month: "long",
          day: "numeric",
          year: "numeric",
        })
      : view === "week"
        ? `Week of ${cursorDate.toLocaleDateString(undefined, { month: "short", day: "numeric" })}`
        : formatMonthYear(cursorDate);

  return (
    <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
      <div className="flex items-center gap-2">
        <h2 className="text-lg font-semibold tracking-tight text-zinc-100">
          {title}
        </h2>
        <button
          type="button"
          onClick={onToday}
          className="rounded-lg border border-zinc-700 px-2.5 py-1 text-xs font-medium text-zinc-300 transition hover:bg-zinc-800"
        >
          Today
        </button>
        <div className="flex items-center gap-0.5">
          <button
            type="button"
            onClick={onPrev}
            className="flex h-8 w-8 items-center justify-center rounded-lg text-zinc-400 transition hover:bg-zinc-800 hover:text-zinc-200"
            aria-label="Previous"
          >
            <CaretLeft size={18} />
          </button>
          <button
            type="button"
            onClick={onNext}
            className="flex h-8 w-8 items-center justify-center rounded-lg text-zinc-400 transition hover:bg-zinc-800 hover:text-zinc-200"
            aria-label="Next"
          >
            <CaretRight size={18} />
          </button>
        </div>
      </div>

      <div className="flex items-center gap-2">
        <div className="flex rounded-xl border border-zinc-800 bg-zinc-950/60 p-0.5">
          {VIEWS.map((v) => (
            <button
              key={v.id}
              type="button"
              onClick={() => onViewChange(v.id)}
              className={`rounded-lg px-3 py-1.5 text-xs font-medium transition ${
                view === v.id
                  ? "bg-zinc-800 text-zinc-100 shadow-sm"
                  : "text-zinc-500 hover:text-zinc-300"
              }`}
            >
              {v.label}
            </button>
          ))}
        </div>
        <button
          type="button"
          onClick={onToggleNotifications}
          className="relative flex h-9 w-9 items-center justify-center rounded-xl border border-zinc-800 text-zinc-400 transition hover:bg-zinc-800 hover:text-zinc-200"
          aria-label="Notifications"
        >
          <Bell size={18} />
          {notificationCount > 0 && (
            <span className="absolute -right-1 -top-1 flex h-4 min-w-4 items-center justify-center rounded-full bg-blue-500 px-1 text-[10px] font-medium text-white">
              {notificationCount > 9 ? "9+" : notificationCount}
            </span>
          )}
        </button>
      </div>
    </div>
  );
}
