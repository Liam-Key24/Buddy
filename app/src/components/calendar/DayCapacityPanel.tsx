import type { DaySummary } from "@buddy/calendar/models";

/** Compact daily capacity + suggestion strip for day view. */
export function DayCapacityPanel({
  summary,
  loading,
}: {
  summary: DaySummary | null;
  loading?: boolean;
}) {
  if (loading && !summary) {
    return (
      <div className="mb-3 rounded-xl border border-zinc-800 bg-zinc-900/50 px-3 py-2 text-xs text-zinc-500">
        Loading capacity…
      </div>
    );
  }
  if (!summary) return null;

  const { capacity, suggestions, conflicts } = summary;
  const metrics = [
    { label: "Booked", value: capacity.booked_hours },
    { label: "Focus", value: capacity.focus_hours },
    { label: "Meetings", value: capacity.meeting_hours },
    { label: "Free", value: capacity.free_hours },
  ];

  return (
    <div
      className={`mb-3 rounded-xl border px-3 py-2.5 ${
        capacity.overloaded
          ? "border-amber-500/40 bg-amber-500/10"
          : "border-zinc-800 bg-zinc-900/50"
      }`}
    >
      <div className="mb-2 flex items-center justify-between gap-2">
        <span className="text-[10px] font-semibold uppercase tracking-wider text-zinc-500">
          Today · capacity
        </span>
        {capacity.overloaded && (
          <span className="rounded-md bg-amber-500/20 px-1.5 py-0.5 text-[10px] font-medium text-amber-200">
            Overloaded
          </span>
        )}
      </div>
      <div className="mb-2 grid grid-cols-4 gap-2">
        {metrics.map((m) => (
          <div key={m.label} className="min-w-0">
            <div className="text-[10px] uppercase tracking-wide text-zinc-500">
              {m.label}
            </div>
            <div className="truncate text-sm font-medium text-zinc-200">
              {m.value.toFixed(1)}h
            </div>
          </div>
        ))}
      </div>
      {conflicts.length > 0 && (
        <p className="mb-1 text-[11px] text-rose-300">
          {conflicts.length} conflict{conflicts.length === 1 ? "" : "s"} detected
        </p>
      )}
      {suggestions.slice(0, 3).map((s, i) => (
        <p key={i} className="truncate text-[11px] text-zinc-400">
          · {s.message}
        </p>
      ))}
    </div>
  );
}
