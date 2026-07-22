import { useState } from "react";
import { Briefcase, X } from "@phosphor-icons/react";
import type { ScheduleBlock, WorkDayLog, WorkStats } from "@buddy/calendar/models";

function fmtHours(h: number) {
  return `${h.toFixed(1)}h`;
}

function fmtMoney(amount: number, currency: string) {
  try {
    return new Intl.NumberFormat(undefined, {
      style: "currency",
      currency: currency || "GBP",
      maximumFractionDigits: 0,
    }).format(amount);
  } catch {
    return `${currency} ${amount.toFixed(0)}`;
  }
}

export function WorkDashboardPanel({
  block,
  stats,
  dayLog,
  loading,
  onClose,
  onSaveSales,
  onSaveEndTime,
}: {
  block: ScheduleBlock;
  stats: WorkStats | null;
  dayLog: WorkDayLog | null;
  loading: boolean;
  onClose: () => void;
  onSaveSales: (amount: number) => Promise<void>;
  onSaveEndTime: (endMs: number) => Promise<void>;
}) {
  const [salesDraft, setSalesDraft] = useState(
    dayLog?.sales_amount ? String(dayLog.sales_amount) : "",
  );
  const [endDraft, setEndDraft] = useState("");
  const [saving, setSaving] = useState(false);

  async function saveSales() {
    const amount = Number(salesDraft);
    if (Number.isNaN(amount)) return;
    setSaving(true);
    try {
      await onSaveSales(amount);
    } finally {
      setSaving(false);
    }
  }

  async function saveEnd() {
    const m = endDraft.trim().match(/^(\d{1,2}):(\d{2})$/);
    if (!m) return;
    const d = new Date(block.anchor_date + "T00:00:00");
    d.setHours(Number(m[1]), Number(m[2]), 0, 0);
    setSaving(true);
    try {
      await onSaveEndTime(d.getTime());
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="fixed inset-y-0 right-0 z-40 flex w-full max-w-sm flex-col border-l border-zinc-800 bg-zinc-900 shadow-2xl shadow-black/40 animate-[slideIn_0.2s_ease-out]">
      <div className="flex items-center justify-between border-b border-zinc-800 px-4 py-3">
        <div className="flex items-center gap-2">
          <Briefcase size={18} className="text-orange-400" />
          <h3 className="text-sm font-semibold text-zinc-100">Work Dashboard</h3>
        </div>
        <button
          type="button"
          onClick={onClose}
          className="rounded-lg p-1 text-zinc-500 hover:bg-zinc-800 hover:text-zinc-300"
        >
          <X size={18} />
        </button>
      </div>
      <div className="border-b border-zinc-800 px-4 py-3">
        <p className="text-xs text-zinc-500">Work day</p>
        <p className="text-sm text-zinc-200">{block.anchor_date}</p>
        <p className="mt-0.5 text-[11px] text-zinc-600">Schedule 8:45 – 16:45</p>
      </div>
      <div className="flex-1 space-y-5 overflow-y-auto p-4">
        {loading || !stats ? (
          <p className="text-xs text-zinc-600">Loading stats…</p>
        ) : (
          <>
            <section>
              <h4 className="mb-2 text-[10px] font-medium uppercase tracking-wider text-zinc-500">
                Hours
              </h4>
              <div className="grid grid-cols-3 gap-2">
                {(
                  [
                    ["Today", stats.today.hours],
                    ["Week", stats.week.hours],
                    ["Month", stats.month.hours],
                  ] as const
                ).map(([label, value]) => (
                  <div
                    key={label}
                    className="rounded-xl border border-zinc-800 bg-zinc-950/50 px-2 py-3 text-center"
                  >
                    <div className="text-[10px] text-zinc-500">{label}</div>
                    <div className="mt-1 text-sm font-semibold text-orange-300">
                      {fmtHours(value)}
                    </div>
                  </div>
                ))}
              </div>
            </section>
            <section>
              <h4 className="mb-2 text-[10px] font-medium uppercase tracking-wider text-zinc-500">
                Sales
              </h4>
              <div className="grid grid-cols-3 gap-2">
                {(
                  [
                    ["Today", stats.today.sales, stats.today.currency],
                    ["Week", stats.week.sales, stats.week.currency],
                    ["Month", stats.month.sales, stats.month.currency],
                  ] as const
                ).map(([label, value, currency]) => (
                  <div
                    key={label}
                    className="rounded-xl border border-zinc-800 bg-zinc-950/50 px-2 py-3 text-center"
                  >
                    <div className="text-[10px] text-zinc-500">{label}</div>
                    <div className="mt-1 text-sm font-semibold text-zinc-100">
                      {fmtMoney(value, currency)}
                    </div>
                  </div>
                ))}
              </div>
            </section>
          </>
        )}
        <section>
          <h4 className="mb-2 text-[10px] font-medium uppercase tracking-wider text-zinc-500">
            Update today
          </h4>
          <label className="mb-1 block text-xs text-zinc-500">Sales (£)</label>
          <div className="flex gap-2">
            <input
              value={salesDraft}
              onChange={(e) => setSalesDraft(e.target.value)}
              className="flex-1 rounded-xl border border-zinc-800 bg-zinc-950/60 px-3 py-2 text-sm text-zinc-200 outline-none focus:border-orange-500/40"
              placeholder="320"
            />
            <button
              type="button"
              disabled={saving}
              onClick={() => void saveSales()}
              className="rounded-xl bg-orange-500 px-3 py-2 text-sm font-medium text-white disabled:opacity-40"
            >
              Save
            </button>
          </div>
          <label className="mb-1 mt-3 block text-xs text-zinc-500">
            Finished at (24h)
          </label>
          <div className="flex gap-2">
            <input
              value={endDraft}
              onChange={(e) => setEndDraft(e.target.value)}
              className="flex-1 rounded-xl border border-zinc-800 bg-zinc-950/60 px-3 py-2 text-sm text-zinc-200 outline-none focus:border-orange-500/40"
              placeholder="17:15"
            />
            <button
              type="button"
              disabled={saving}
              onClick={() => void saveEnd()}
              className="rounded-xl bg-zinc-800 px-3 py-2 text-sm font-medium text-zinc-200 disabled:opacity-40"
            >
              Set
            </button>
          </div>
        </section>
      </div>
    </div>
  );
}
