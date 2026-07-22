import { useState } from "react";
import { MoonStars, Trash, X } from "@phosphor-icons/react";
import type { DreamEntry, ScheduleBlock } from "@buddy/calendar/models";

export function DreamLogPanel({
  block,
  dreams,
  loading,
  onClose,
  onAdd,
  onDelete,
}: {
  block: ScheduleBlock;
  dreams: DreamEntry[];
  loading: boolean;
  onClose: () => void;
  onAdd: (body: string) => Promise<void>;
  onDelete: (id: string) => Promise<void>;
}) {
  const [draft, setDraft] = useState("");
  const [saving, setSaving] = useState(false);

  async function submit() {
    const body = draft.trim();
    if (!body) return;
    setSaving(true);
    try {
      await onAdd(body);
      setDraft("");
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="fixed inset-y-0 right-0 z-40 flex w-full max-w-sm flex-col border-l border-zinc-800 bg-zinc-900 shadow-2xl shadow-black/40 animate-[slideIn_0.2s_ease-out]">
      <div className="flex items-center justify-between border-b border-zinc-800 px-4 py-3">
        <div className="flex items-center gap-2">
          <MoonStars size={18} className="text-indigo-400" />
          <h3 className="text-sm font-semibold text-zinc-100">Dream Log</h3>
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
        <p className="text-xs text-zinc-500">Sleep night</p>
        <p className="text-sm text-zinc-200">{block.anchor_date}</p>
        <p className="mt-0.5 text-[11px] text-zinc-600">
          {new Date(block.start_time).toLocaleTimeString([], {
            hour: "numeric",
            minute: "2-digit",
          })}{" "}
          –{" "}
          {new Date(block.end_time).toLocaleTimeString([], {
            hour: "numeric",
            minute: "2-digit",
          })}
        </p>
      </div>
      <div className="flex-1 space-y-3 overflow-y-auto p-4">
        {loading ? (
          <p className="text-xs text-zinc-600">Loading dreams…</p>
        ) : dreams.length === 0 ? (
          <p className="text-xs text-zinc-600">No dreams logged for this night.</p>
        ) : (
          dreams.map((d) => (
            <div
              key={d.id}
              className="rounded-xl border border-zinc-800 bg-zinc-950/50 p-3"
            >
              <div className="mb-1 flex items-start justify-between gap-2">
                <div className="text-xs font-medium text-zinc-300">
                  {d.title || "Dream"}
                </div>
                <button
                  type="button"
                  onClick={() => void onDelete(d.id)}
                  className="text-zinc-600 hover:text-red-400"
                  aria-label="Delete dream"
                >
                  <Trash size={14} />
                </button>
              </div>
              <p className="whitespace-pre-wrap text-sm text-zinc-400">{d.body}</p>
              {d.tags.length > 0 && (
                <div className="mt-2 flex flex-wrap gap-1">
                  {d.tags.map((t) => (
                    <span
                      key={t}
                      className="rounded-full bg-indigo-500/15 px-2 py-0.5 text-[10px] text-indigo-300"
                    >
                      {t}
                    </span>
                  ))}
                </div>
              )}
            </div>
          ))
        )}
      </div>
      <div className="border-t border-zinc-800 p-4">
        <textarea
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          placeholder="Describe a dream…"
          rows={3}
          className="w-full resize-none rounded-xl border border-zinc-800 bg-zinc-950/60 px-3 py-2 text-sm text-zinc-200 outline-none placeholder:text-zinc-600 focus:border-indigo-500/40"
        />
        <button
          type="button"
          disabled={saving || !draft.trim()}
          onClick={() => void submit()}
          className="mt-2 w-full rounded-xl bg-indigo-500 px-3 py-2 text-sm font-medium text-white disabled:opacity-40"
        >
          {saving ? "Saving…" : "Add dream"}
        </button>
      </div>
    </div>
  );
}
