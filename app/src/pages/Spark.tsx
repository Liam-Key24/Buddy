import { useEffect, useState, type ComponentType, type ReactNode } from "react";
import {
  AirplaneTilt,
  Archive,
  ChatCircle,
  Clock,
  Cube,
  House,
  Lightning,
  Plus,
  Trash,
  Tree,
  Van,
} from "@phosphor-icons/react";
import { useAppStore } from "../stores/useAppStore";
import { useChatStore } from "../stores/useChatStore";
import { createConversation } from "../lib/api";
import {
  SPARK_CATEGORIES,
  type Spark,
  type SparkTagId,
  categoryConfig,
  filterSparksByTag,
  formatSparkDate,
  tagLabel,
  useSparkStore,
} from "../stores/useSparkStore";

const CAT_ICONS: Record<
  SparkTagId,
  ComponentType<{ size?: number; weight?: "fill" | "regular"; className?: string }>
> = {
  projects: Cube,
  the_land: Tree,
  the_van: Van,
  general_life: House,
  travelling: AirplaneTilt,
};

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
  const [adding, setAdding] = useState(false);
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
    setAdding(false);
  }

  async function openInChat(content: string) {
    const conv = await createConversation("Spark");
    setActiveConversationId(conv.id);
    setMessages([]);
    setPendingChatMessage(`Let's develop this spark: ${content}`);
    setCurrentPage("chat");
  }

  return (
    <div className="min-h-0 flex-1 overflow-y-auto p-4">
      <div className="mx-auto max-w-5xl space-y-4">
        <div className="flex items-center gap-2">
          <Lightning size={18} weight="fill" className="text-amber-400" />
          <div className="min-w-0 flex-1">
            <h3 className="text-sm font-medium text-zinc-200">Sparks</h3>
            {staleCount > 0 && (
              <p className="text-xs text-amber-400">{staleCount} need attention</p>
            )}
          </div>
          <button
            type="button"
            title="Add spark"
            aria-label="Add spark"
            onClick={() => setAdding((v) => !v)}
            className="rounded-md p-1 text-blue-400 hover:bg-zinc-800"
          >
            <Plus size={16} weight="bold" />
          </button>
        </div>

        <div className="flex items-center gap-0.5" role="group" aria-label="Filter">
          <button
            type="button"
            title="All"
            aria-label="All"
            aria-pressed={tagFilter === null}
            onClick={() => setTagFilter(null)}
            className={`rounded-md p-1.5 ${
              tagFilter === null ? "text-blue-400" : "text-zinc-600 hover:text-zinc-300"
            }`}
          >
            <Lightning size={16} weight={tagFilter === null ? "fill" : "regular"} />
          </button>
          {SPARK_CATEGORIES.map((cat) => {
            const Icon = CAT_ICONS[cat.id];
            const selected = tagFilter === cat.id;
            return (
              <button
                key={cat.id}
                type="button"
                title={cat.label}
                aria-label={cat.label}
                aria-pressed={selected}
                onClick={() => setTagFilter(selected ? null : cat.id)}
                className={`rounded-md p-1.5 ${
                  selected ? cat.iconColor : "text-zinc-600 hover:text-zinc-300"
                }`}
              >
                <Icon size={16} weight={selected ? "fill" : "regular"} />
              </button>
            );
          })}
        </div>

        {adding && (
          <div className="rounded-xl border border-zinc-800 bg-zinc-950/40 px-3 py-2.5">
            <textarea
              value={newContent}
              onChange={(e) => setNewContent(e.target.value)}
              placeholder="What's the idea?"
              rows={2}
              className="w-full resize-none bg-transparent text-sm text-zinc-100 outline-none"
            />
            <div className="mt-2 flex items-center justify-between gap-2">
              <div className="flex items-center gap-0.5">
                {SPARK_CATEGORIES.map((cat) => {
                  const Icon = CAT_ICONS[cat.id];
                  const selected = newTags.includes(cat.id);
                  return (
                    <button
                      key={cat.id}
                      type="button"
                      title={cat.label}
                      aria-label={cat.label}
                      aria-pressed={selected}
                      onClick={() => toggleTag(cat.id)}
                      className={`rounded-md p-1 ${
                        selected ? cat.iconColor : "text-zinc-600 hover:text-zinc-300"
                      }`}
                    >
                      <Icon size={15} weight={selected ? "fill" : "regular"} />
                    </button>
                  );
                })}
              </div>
              <button
                type="button"
                onClick={() => void handleAdd()}
                disabled={!newContent.trim() || newTags.length === 0}
                className="rounded-lg bg-blue-500 px-2.5 py-1 text-xs text-white disabled:opacity-40"
              >
                Save
              </button>
            </div>
          </div>
        )}

        {loading ? (
          <p className="px-1 text-xs text-zinc-600">Loading…</p>
        ) : filtered.length === 0 ? (
          <p className="px-1 text-xs text-zinc-600">No sparks yet</p>
        ) : (
          <div className="grid grid-cols-2 gap-2">
            {filtered.map((spark) => (
              <SparkCard
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
  );
}

function SparkCard({
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
  const primary = (spark.tags[0] as SparkTagId | undefined) ?? "general_life";
  const cfg = categoryConfig(primary);
  const Icon = CAT_ICONS[primary] ?? House;

  return (
    <div
      className={`flex items-start gap-2.5 rounded-xl border bg-zinc-950/40 px-3 py-2.5 ${
        stale ? "border-amber-500/40" : "border-zinc-800"
      }`}
    >
      <span
        title={tagLabel(primary)}
        className={`flex h-8 w-8 shrink-0 items-center justify-center rounded-lg ${
          cfg ? `${cfg.chip}` : "bg-zinc-800 text-zinc-400"
        }`}
      >
        <Icon size={16} weight="fill" />
      </span>
      <div className="min-w-0 flex-1">
        <p className="text-sm text-zinc-100">{spark.content}</p>
        <div className="mt-1 flex items-center gap-2 text-[11px] text-zinc-500">
          <span className="flex items-center gap-1">
            <Clock size={12} />
            {formatSparkDate(spark.created_at)}
          </span>
          {stale && <span className="text-amber-400">Needs attention</span>}
          {spark.tags.length > 1 && (
            <span className="flex items-center gap-0.5">
              {spark.tags.slice(1).map((t) => {
                const Extra = CAT_ICONS[t as SparkTagId] ?? House;
                return (
                  <Extra
                    key={t}
                    size={11}
                    weight="fill"
                    className={categoryConfig(t)?.iconColor ?? "text-zinc-500"}
                  />
                );
              })}
            </span>
          )}
        </div>
      </div>
      <div className="flex shrink-0 items-center gap-0.5">
        <IconBtn title="Re-spark" onClick={onRespark}>
          <Lightning size={14} />
        </IconBtn>
        <IconBtn title="Chat" onClick={onChat}>
          <ChatCircle size={14} />
        </IconBtn>
        <IconBtn title="Archive" onClick={onArchive}>
          <Archive size={14} />
        </IconBtn>
        <IconBtn title="Delete" danger onClick={onDelete}>
          <Trash size={14} />
        </IconBtn>
      </div>
    </div>
  );
}

function IconBtn({
  title,
  onClick,
  danger,
  children,
}: {
  title: string;
  onClick: () => void;
  danger?: boolean;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      title={title}
      aria-label={title}
      onClick={onClick}
      className={`rounded-md p-1 ${
        danger
          ? "text-zinc-600 hover:bg-zinc-800 hover:text-rose-400"
          : "text-zinc-600 hover:bg-zinc-800 hover:text-zinc-200"
      }`}
    >
      {children}
    </button>
  );
}
