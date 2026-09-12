import { useEffect, useState } from "react";
import {
  FileCsv,
  FileHtml,
  FileText,
  FloppyDisk,
  MagnifyingGlass,
  PushPin,
  Trash,
} from "@phosphor-icons/react";
import { LifeChatSplit } from "../components/LifeChatSplit";
import { ToolWorkspace } from "../components/ToolWorkspace";
import { WorkspacePaneHeader } from "../components/life/WorkspaceChrome";
import { CsvEditor, RichDocumentEditor } from "../components/documents/DocumentEditor";
import { useLifePage } from "../hooks/useLifePage";
import { useAppStore } from "../stores/useAppStore";
import {
  docsDelete,
  docsGet,
  docsList,
  docsSearch,
  docsUpsert,
  subscribeLifeEvent,
  type DocumentRow,
} from "../lib/lifeApi";

export function Documents() {
  const [docs, setDocs] = useState<DocumentRow[]>([]);
  const [openIds, setOpenIds] = useState<string[]>([]);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [active, setActive] = useState<DocumentRow | null>(null);
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<{ id: string; title: string; snippet: string }[]>([]);

  async function reload() {
    setDocs(await docsList(null));
  }

  const pendingWorkspaceDocId = useAppStore((s) => s.pendingWorkspaceDocId);
  const setPendingWorkspaceDocId = useAppStore((s) => s.setPendingWorkspaceDocId);
  const open = active ?? docs.find((d) => d.id === activeId) ?? null;

  useLifePage({
    load: reload,
    context: open
      ? `Page: documents. Open document id=${open.id} title="${open.title}" format=${open.format}. Reformat with docs.format (pass id or title). Small edits: docs.patch. New body: docs.upsert.`
      : "Page: documents. No document open. Create with docs.upsert / docs.search, or reformat by title with docs.format.",
  });

  useEffect(() => {
    if (!pendingWorkspaceDocId) return;
    setOpenIds((ids) =>
      ids.includes(pendingWorkspaceDocId) ? ids : [...ids, pendingWorkspaceDocId],
    );
    setActiveId(pendingWorkspaceDocId);
    setPendingWorkspaceDocId(null);
  }, [pendingWorkspaceDocId, setPendingWorkspaceDocId]);

  useEffect(() => {
    return subscribeLifeEvent("docs-updated", async () => {
      const d = await docsList(null);
      setDocs(d);
      const newest = [...d].sort((a, b) => b.updated_at - a.updated_at)[0];
      const nowSec = Date.now() / 1000;
      if (newest && nowSec - newest.updated_at < 30) {
        setOpenIds((ids) => (ids.includes(newest.id) ? ids : [...ids, newest.id]));
        setActiveId(newest.id);
        docsGet(newest.id).then(setActive).catch(console.error);
        return;
      }
      if (activeId) {
        docsGet(activeId).then(setActive).catch(console.error);
      }
    });
  }, [activeId]);

  useEffect(() => {
    if (!activeId) {
      setActive(null);
      return;
    }
    docsGet(activeId).then(setActive).catch(console.error);
  }, [activeId]);

  function openDoc(id: string) {
    setOpenIds((ids) => (ids.includes(id) ? ids : [...ids, id]));
    setActiveId(id);
  }

  function closeTab(id: string) {
    setOpenIds((ids) => ids.filter((x) => x !== id));
    if (activeId === id) {
      const next = openIds.filter((x) => x !== id);
      setActiveId(next[next.length - 1] ?? null);
    }
  }

  async function saveActive(patch: Partial<DocumentRow>) {
    if (!active) return;
    const next = { ...active, ...patch };
    const saved = await docsUpsert({
      id: next.id,
      folderId: next.folder_id,
      title: next.title,
      format: next.format,
      content: next.content,
      pinned: next.pinned,
    });
    setActive(saved);
    await reload();
  }

  async function search(q: string) {
    setQuery(q);
    if (!q.trim()) {
      setHits([]);
      return;
    }
    setHits(await docsSearch(q));
  }

  return (
    <LifeChatSplit>
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <ToolWorkspace
        narrow
        sidebar={
          <>
            <div className="relative">
              <MagnifyingGlass size={12} className="pointer-events-none absolute left-2 top-1/2 -translate-y-1/2 text-zinc-500" />
              <input
                value={query}
                onChange={(e) => search(e.target.value)}
                placeholder="Search"
                className="w-full rounded-xl border border-zinc-800 bg-zinc-950 py-1.5 pl-7 pr-2 text-xs text-zinc-200 outline-none"
              />
            </div>
            {hits.length > 0 && (
              <div className="space-y-0.5">
                {hits.map((h) => (
                  <button
                    key={h.id}
                    type="button"
                    onClick={() => openDoc(h.id)}
                    className="block w-full truncate rounded-lg px-2 py-1 text-left text-xs text-zinc-300 hover:bg-zinc-800"
                  >
                    {h.title}
                  </button>
                ))}
              </div>
            )}
            <div className="min-h-0 flex-1 overflow-y-auto [scrollbar-width:none] [&::-webkit-scrollbar]:w-0">
              {docs.map((d) => (
                <DocLink key={d.id} doc={d} active={activeId === d.id} onOpen={() => openDoc(d.id)} />
              ))}
            </div>
          </>
        }
      >
        <WorkspacePaneHeader Icon={FileText}>
          {active ? (
            <>
              <input
                value={active.title}
                onChange={(e) => setActive({ ...active, title: e.target.value })}
                onBlur={() => saveActive({ title: active.title })}
                className="min-w-0 flex-1 bg-transparent text-sm font-medium text-zinc-100 outline-none"
              />
              <button
                type="button"
                title={active.pinned ? "Unpin" : "Pin"}
                aria-label={active.pinned ? "Unpin" : "Pin"}
                onClick={() => saveActive({ pinned: !active.pinned })}
                className={`rounded-md p-1 ${active.pinned ? "text-amber-400" : "text-zinc-500 hover:bg-zinc-800"}`}
              >
                <PushPin size={15} weight={active.pinned ? "fill" : "regular"} />
              </button>
              {active.format !== "csv" && (
                <button
                  type="button"
                  title="Save"
                  aria-label="Save"
                  onClick={() => saveActive({ content: active.content })}
                  className="rounded-md p-1 text-zinc-500 hover:bg-zinc-800 hover:text-zinc-200"
                >
                  <FloppyDisk size={15} />
                </button>
              )}
              <button
                type="button"
                title="Delete"
                aria-label="Delete"
                onClick={async () => {
                  await docsDelete(active.id);
                  closeTab(active.id);
                  await reload();
                }}
                className="rounded-md p-1 text-zinc-500 hover:bg-zinc-800 hover:text-rose-400"
              >
                <Trash size={15} />
              </button>
            </>
          ) : (
            <h3 className="text-sm font-medium text-zinc-200">Documents</h3>
          )}
        </WorkspacePaneHeader>
        {openIds.length > 0 && (
          <div className="mb-2 flex gap-1 overflow-x-auto [scrollbar-width:none] [&::-webkit-scrollbar]:w-0">
            {openIds.map((id) => {
              const d = docs.find((x) => x.id === id);
              return (
                <button
                  key={id}
                  type="button"
                  onClick={() => setActiveId(id)}
                  className={`flex items-center gap-1 rounded-lg px-2 py-1 text-xs ${activeId === id ? "bg-blue-500/15 text-blue-400" : "text-zinc-400 hover:bg-zinc-800"}`}
                >
                  {d?.title ?? "…"}
                  <span
                    onClick={(e) => {
                      e.stopPropagation();
                      closeTab(id);
                    }}
                    className="text-zinc-600 hover:text-zinc-300"
                  >
                    ×
                  </span>
                </button>
              );
            })}
          </div>
        )}
        {active ? (
          <div className="flex min-h-0 flex-1 flex-col">
            {active.format === "csv" ? (
              <CsvEditor content={active.content} onChange={(content) => saveActive({ content })} />
            ) : (
              <RichDocumentEditor
                content={active.content}
                onChange={(content) => {
                  setActive({ ...active, content });
                }}
              />
            )}
          </div>
        ) : (
          <div className="min-h-0 flex-1 rounded-xl border border-dashed border-zinc-800" />
        )}
      </ToolWorkspace>
    </div>
    </LifeChatSplit>
  );
}

function FormatIcon({ format }: { format: string }) {
  if (format === "csv") return <FileCsv size={12} />;
  if (format === "html") return <FileHtml size={12} />;
  return <FileText size={12} />;
}

function DocLink({ doc, active, onOpen }: { doc: DocumentRow; active: boolean; onOpen: () => void }) {
  return (
    <button
      type="button"
      onClick={onOpen}
      title={doc.title}
      className={`mb-0.5 flex w-full items-center gap-1.5 rounded-lg px-2 py-1.5 text-left text-xs ${active ? "bg-blue-500/15 text-blue-400" : "text-zinc-400 hover:bg-zinc-800"}`}
    >
      <FormatIcon format={doc.format} />
      {doc.pinned && <PushPin size={10} weight="fill" className="shrink-0 text-amber-400" />}
      <span className="truncate">{doc.title}</span>
    </button>
  );
}
