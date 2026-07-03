import { useEffect, useState } from "react";
import { Lightning, Plus, X } from "@phosphor-icons/react";
import { useAppStore } from "../stores/useAppStore";
import { useChatStore } from "../stores/useChatStore";
import { createConversation } from "../lib/api";
import {
  SPARK_CATEGORIES,
  type Spark,
  SparkTagId,
  categoryConfig,
  filterSparksByTag,
  formatSparkDate,
  tagLabel,
  useSparkStore,
} from "../stores/useSparkStore";

function ActionBtn({
  label,
  onClick,
  primary,
  danger,
}: {
  label: string;
  onClick: () => void;
  primary?: boolean;
  danger?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`rounded-md px-2 py-0.5 text-[11px] font-medium transition ${
        danger
          ? "bg-rose-500/15 text-rose-400 hover:bg-rose-500/25"
          : primary
            ? "bg-blue-500/20 text-blue-400 hover:bg-blue-500/30"
            : "bg-zinc-700/60 text-zinc-300 hover:bg-zinc-700"
      }`}
    >
      {label}
    </button>
  );
}

function TagChip({ tag }: { tag: string }) {
  const cfg = categoryConfig(tag);
  return (
    <span
      className={`rounded-full px-2 py-0.5 text-[10px] font-medium ${cfg?.chip ?? "bg-zinc-700/60 text-zinc-400"}`}
    >
      {tagLabel(tag)}
    </span>
  );
}

function SparkRow({
  spark,
  stale,
  onRespark,
  onArchive,
  onDelete,
  onChat,
}: {
  spark: Spark;
  stale: boolean;
  onRespark: () => void;
  onArchive: () => void;
  onDelete: () => void;
  onChat: () => void;
}) {
  return (
    <div
      className={`group grid grid-cols-[1fr_7rem_11rem] items-center gap-4 px-4 py-3.5 ${
        stale ? "border-l-2 border-amber-500/50 bg-amber-500/5" : ""
      }`}
    >
      <div className="min-w-0">
        <p className="line-clamp-2 text-sm text-zinc-200">{spark.content}</p>
        {stale && (
          <span className="mt-0.5 text-[10px] text-amber-400">Needs attention</span>
        )}
      </div>
      <p className="text-xs text-zinc-500">{formatSparkDate(spark.created_at)}</p>
      <div className="flex items-center justify-end gap-2">
        <div className="flex flex-wrap justify-end gap-1 group-hover:hidden">
          {spark.tags.map((t) => (
            <TagChip key={t} tag={t} />
          ))}
        </div>
        <div className="hidden shrink-0 flex-wrap justify-end gap-1 group-hover:flex">
          <ActionBtn label="Re-spark" onClick={onRespark} />
          <ActionBtn label="Archive" onClick={onArchive} />
          <ActionBtn label="Chat" primary onClick={onChat} />
          <ActionBtn label="Delete" danger onClick={onDelete} />
        </div>
      </div>
    </div>
  );
}

export function Spark() {
  const {
    sparks,
    staleSparks,
    staleCount,
    loading,
    refresh,
    addSpark,
    respark,
    archiveSpark,
    deleteSpark,
  } = useSparkStore();
  const { setCurrentPage, setPendingChatMessage } = useAppStore();
  const { setActiveConversationId, setMessages } = useChatStore();

  const [tagFilter, setTagFilter] = useState<SparkTagId | null>(null);
  const [showAdd, setShowAdd] = useState(false);
  const [newContent, setNewContent] = useState("");
  const [newTags, setNewTags] = useState<SparkTagId[]>(["general_life"]);

  useEffect(() => {
    refresh();
  }, []);

  const staleIds = new Set(staleSparks.map((s) => s.id));
  const filtered = filterSparksByTag(sparks, tagFilter);

  function toggleTag(tag: SparkTagId) {
    setNewTags((prev) =>
      prev.includes(tag) ? prev.filter((t) => t !== tag) : [...prev, tag],
    );
  }

  async function handleAdd() {
    const trimmed = newContent.trim();
    if (!trimmed || newTags.length === 0) return;
    await addSpark(trimmed, newTags);
    setNewContent("");
    setNewTags(["general_life"]);
    setShowAdd(false);
  }

  async function openInChat(content: string) {
    const conv = await createConversation("Spark");
    setActiveConversationId(conv.id);
    setMessages([]);
    setPendingChatMessage(`Let's develop this spark: ${content}`);
    setCurrentPage("chat");
  }

  return (
    <div className="flex-1 overflow-y-auto p-6">
      <div className="mx-auto max-w-5xl space-y-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Lightning size={18} weight="duotone" className="text-amber-400" />
            <p className="text-sm text-zinc-400">
              Sparks ({sparks.length})
              {staleCount > 0 && (
                <span className="ml-2 text-amber-400">
                  · {staleCount} need attention
                </span>
              )}
            </p>
          </div>
          <button
            type="button"
            onClick={() => setShowAdd(!showAdd)}
            className="flex items-center gap-1.5 rounded-lg bg-zinc-800 px-3 py-1.5 text-xs font-medium text-zinc-300 transition hover:bg-zinc-700"
          >
            {showAdd ? <X size={14} /> : <Plus size={14} weight="bold" />}
            {showAdd ? "Cancel" : "Add spark"}
          </button>
        </div>

        {showAdd && (
          <div className="space-y-3 rounded-2xl border border-zinc-800 bg-zinc-900 p-4">
            <textarea
              value={newContent}
              onChange={(e) => setNewContent(e.target.value)}
              placeholder="What's the idea?"
              rows={2}
              className="w-full resize-none rounded-xl border border-zinc-800 bg-zinc-800/50 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-500 outline-none focus:border-zinc-600"
            />
            <div className="flex flex-wrap gap-1.5">
              {SPARK_CATEGORIES.map((cat) => (
                <button
                  key={cat.id}
                  type="button"
                  onClick={() => toggleTag(cat.id)}
                  className={`rounded-full px-2.5 py-1 text-xs transition ${
                    newTags.includes(cat.id)
                      ? cat.chip
                      : "bg-zinc-800 text-zinc-400 hover:bg-zinc-700"
                  }`}
                >
                  {cat.label}
                </button>
              ))}
            </div>
            <button
              type="button"
              onClick={handleAdd}
              disabled={!newContent.trim() || newTags.length === 0}
              className="rounded-lg bg-blue-500 px-4 py-1.5 text-xs font-medium text-white transition hover:bg-blue-600 disabled:opacity-40"
            >
              Save
            </button>
          </div>
        )}

        <div className="flex flex-wrap gap-1.5">
          <button
            type="button"
            onClick={() => setTagFilter(null)}
            className={`rounded-full px-3 py-1 text-xs font-medium transition ${
              tagFilter === null
                ? "bg-blue-500/20 text-blue-400"
                : "bg-zinc-800 text-zinc-400 hover:bg-zinc-700"
            }`}
          >
            All
          </button>
          {SPARK_CATEGORIES.map((cat) => (
            <button
              key={cat.id}
              type="button"
              onClick={() => setTagFilter(cat.id)}
              className={`rounded-full px-3 py-1 text-xs font-medium transition ${
                tagFilter === cat.id
                  ? "bg-blue-500/20 text-blue-400"
                  : "bg-zinc-800 text-zinc-400 hover:bg-zinc-700"
              }`}
            >
              {cat.label}
            </button>
          ))}
        </div>

        <div className="overflow-hidden rounded-2xl border border-zinc-800 bg-zinc-900">
          <div className="grid grid-cols-[1fr_7rem_11rem] gap-4 border-b border-zinc-800 px-4 py-2.5">
            <p className="text-[10px] font-medium uppercase tracking-wider text-zinc-500">
              Idea
            </p>
            <p className="text-[10px] font-medium uppercase tracking-wider text-zinc-500">
              Date created
            </p>
            <p className="text-right text-[10px] font-medium uppercase tracking-wider text-zinc-500">
              Tags
            </p>
          </div>

          {loading ? (
            <p className="py-12 text-center text-sm text-zinc-500">Loading…</p>
          ) : filtered.length === 0 ? (
            <p className="py-12 text-center text-sm text-zinc-500">No sparks yet</p>
          ) : (
            <div className="divide-y divide-zinc-800">
              {filtered.map((spark) => (
                <SparkRow
                  key={spark.id}
                  spark={spark}
                  stale={staleIds.has(spark.id)}
                  onRespark={() => respark(spark.id)}
                  onArchive={() => archiveSpark(spark.id)}
                  onDelete={() => deleteSpark(spark.id)}
                  onChat={() => openInChat(spark.content)}
                />
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
