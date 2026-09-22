import {
  CalendarBlank,
  CaretDown,
  CaretRight,
  ChatCircle,
  FolderPlus,
  FolderSimple,
  FunnelSimple,
  GearSix,
  Lightning,
  MagnifyingGlass,
  PencilSimple,
  Plus,
  SidebarSimple,
  SquaresFour,
  Target,
  Trash,
  X,
} from "@phosphor-icons/react";
import { useMemo, useState, type DragEvent } from "react";
import { NavLink, useLocation } from "react-router-dom";
import { useChatNav } from "../chatNav";
import { cn } from "../lib/cn";
import type { Conversation } from "../api";
import { IconButton } from "./ui/IconButton";
import { IconNavItem } from "./ui/IconNavItem";

const CHAT_DRAG = "application/x-buddy-chat";

type ChatFilter = "newest" | "oldest" | "name" | "unfinished";

const FILTERS: Array<{ id: ChatFilter; label: string }> = [
  { id: "newest", label: "Newest" },
  { id: "oldest", label: "Oldest" },
  { id: "name", label: "Name A–Z" },
  { id: "unfinished", label: "Unfinished" },
];

function relativeTime(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime();
  const m = Math.max(0, Math.round(diff / 60_000));
  if (m < 1) return "now";
  if (m < 60) return `${m}m`;
  const h = Math.round(m / 60);
  if (h < 48) return `${h}h`;
  const d = Math.round(h / 24);
  return `${d}d`;
}

function sortChats(rows: Conversation[], filter: ChatFilter): Conversation[] {
  const copy = [...rows];
  if (filter === "oldest") {
    copy.sort((a, b) => a.updated_at.localeCompare(b.updated_at));
  } else if (filter === "name") {
    copy.sort((a, b) => a.title.localeCompare(b.title, undefined, { sensitivity: "base" }));
  } else {
    copy.sort((a, b) => {
      const ao = a.sort_order ?? 0;
      const bo = b.sort_order ?? 0;
      if (ao !== bo) return ao - bo;
      return b.updated_at.localeCompare(a.updated_at);
    });
  }
  return copy;
}

const routes = [
  { to: "/", label: "Today", icon: SquaresFour, end: true },
  { to: "/goals", label: "Goals", icon: Target },
  { to: "/calendar", label: "Calendar", icon: CalendarBlank },
  { to: "/sparks", label: "Sparks", icon: Lightning },
];

type SharedSidebarProps = {
  collapsed: boolean;
  mobileOpen: boolean;
  onToggleCollapsed: () => void;
  onCloseMobile: () => void;
};

export function SharedSidebar({
  collapsed,
  mobileOpen,
  onToggleCollapsed,
  onCloseMobile,
}: SharedSidebarProps) {
  const compact = collapsed && !mobileOpen;

  return (
    <>
      <div className="hidden h-full md:block">
        <SidebarPanel
          compact={compact}
          onToggleCollapsed={onToggleCollapsed}
          onCloseMobile={onCloseMobile}
        />
      </div>
      {mobileOpen && (
        <div className="fixed inset-0 z-30 md:hidden">
          <button
            type="button"
            className="absolute inset-0 bg-overlay"
            aria-label="Close menu"
            onClick={onCloseMobile}
          />
          <div className="relative h-full w-[min(280px,85vw)]">
            <SidebarPanel
              compact={false}
              onToggleCollapsed={onToggleCollapsed}
              onCloseMobile={onCloseMobile}
            />
          </div>
        </div>
      )}
    </>
  );
}

function SidebarPanel({
  compact,
  onToggleCollapsed,
  onCloseMobile,
}: {
  compact: boolean;
  onToggleCollapsed: () => void;
  onCloseMobile: () => void;
}) {
  const {
    conversations,
    folders,
    conversationId,
    search,
    setSearch,
    selectConversation,
    newChat,
    renameChat,
    deleteChat,
    moveChat,
    placeChat,
    addFolder,
    renameFolderTitle,
    removeFolder,
  } = useChatNav();
  const location = useLocation();
  const [expanded, setExpanded] = useState<Record<string, boolean>>({});
  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const [folderDraft, setFolderDraft] = useState<string | null>(null);
  const [moveFor, setMoveFor] = useState<string | null>(null);
  const [filter, setFilter] = useState<ChatFilter>("newest");
  const [filterOpen, setFilterOpen] = useState(false);
  const [draggingId, setDraggingId] = useState<string | null>(null);
  const [dropTarget, setDropTarget] = useState<string | null>(null);

  const q = search.trim().toLowerCase();
  const filtered = useMemo(() => {
    let rows = conversations.filter((c) => !q || c.title.toLowerCase().includes(q));
    if (filter === "unfinished") {
      rows = rows.filter((c) => (c.user_message_count ?? 0) === 0);
    }
    return sortChats(rows, filter);
  }, [conversations, q, filter]);

  const unfiled = filtered.filter((c) => !c.folder_id);
  const byFolder = useMemo(() => {
    const map = new Map<string, Conversation[]>();
    for (const f of folders) map.set(f.id, []);
    for (const c of filtered) {
      if (!c.folder_id) continue;
      const list = map.get(c.folder_id) || [];
      list.push(c);
      map.set(c.folder_id, list);
    }
    return map;
  }, [filtered, folders]);

  async function commitRename(id: string, kind: "chat" | "folder") {
    const title = renameValue.trim();
    setRenamingId(null);
    if (!title) return;
    if (kind === "chat") await renameChat(id, title);
    else await renameFolderTitle(id, title);
  }

  function onDragStart(e: DragEvent, id: string) {
    e.dataTransfer.setData(CHAT_DRAG, id);
    e.dataTransfer.setData("text/plain", id);
    e.dataTransfer.effectAllowed = "move";
    setDraggingId(id);
  }

  function onDragEnd() {
    setDraggingId(null);
    setDropTarget(null);
  }

  async function dropChat(folderId: string | null, beforeId?: string | null, fromEvent?: DragEvent) {
    const id =
      fromEvent?.dataTransfer.getData(CHAT_DRAG) ||
      fromEvent?.dataTransfer.getData("text/plain") ||
      draggingId;
    if (!id) return;
    setDropTarget(null);
    setDraggingId(null);
    if (id === beforeId) return;
    await placeChat(id, folderId, beforeId);
  }

  const rowProps = (c: Conversation) => ({
    title: c.title,
    time: relativeTime(c.updated_at),
    active: c.id === conversationId && location.pathname === "/chat",
    renaming: renamingId === c.id,
    renameValue,
    moving: moveFor === c.id,
    folders,
    dragging: draggingId === c.id,
    dropActive: dropTarget === c.id,
    onSelect: () => {
      selectConversation(c.id);
      onCloseMobile();
    },
    onRenameStart: () => {
      setRenamingId(c.id);
      setRenameValue(c.title);
    },
    onRenameChange: setRenameValue,
    onRenameCommit: () => void commitRename(c.id, "chat"),
    onDelete: () => void deleteChat(c.id),
    onToggleMove: () => setMoveFor((id) => (id === c.id ? null : c.id)),
    onMove: (folderId: string | null) => {
      void moveChat(c.id, folderId);
      setMoveFor(null);
    },
    onDragStart: (e: DragEvent) => onDragStart(e, c.id),
    onDragEnd,
    onDragOver: (e: DragEvent) => {
      e.preventDefault();
      setDropTarget(c.id);
    },
    onDrop: (e: DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
              void dropChat(c.folder_id ?? null, c.id, e);
    },
  });

  return (
    <aside
      className={cn(
        "flex h-full flex-col bg-sidebar text-ink",
        compact ? "w-[68px] px-2 py-3" : "w-[260px] px-3 py-4",
      )}
      aria-label="Primary"
    >
      <div className={cn("mb-3 flex items-center", compact ? "justify-center" : "justify-between")}>
        {!compact && (
          <img src="/buddy-icon.png" alt="Buddy" width={28} height={28} className="rounded-lg" />
        )}
        <IconButton
          label={compact ? "Expand sidebar" : "Collapse sidebar"}
          onClick={onToggleCollapsed}
          className="hidden md:inline-flex"
        >
          <SidebarSimple size={18} />
        </IconButton>
        <IconButton label="Close menu" onClick={onCloseMobile} className="md:hidden">
          <X size={18} />
        </IconButton>
      </div>

      <div className="flex flex-col gap-0.5">
        <IconNavItem
          icon={<Plus size={18} weight="bold" />}
          label="New Chat"
          collapsed={compact}
          onClick={() => {
            void newChat();
            onCloseMobile();
          }}
        />
        {!compact ? (
          <label className="mt-1 flex items-center gap-2 rounded-xl bg-sidebar-hover px-2.5 py-1.5 text-sm text-muted">
            <MagnifyingGlass size={16} />
            <input
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Search"
              aria-label="Search chats"
              className="min-w-0 flex-1 bg-transparent outline-none placeholder:text-muted-dim"
            />
          </label>
        ) : (
          <IconNavItem
            icon={<MagnifyingGlass size={18} />}
            label="Search"
            collapsed
            onClick={onToggleCollapsed}
          />
        )}
        {routes.map((r) => {
          const Icon = r.icon;
          return (
            <IconNavItem
              key={r.to}
              to={r.to}
              end={r.end}
              icon={<Icon size={18} />}
              label={r.label}
              collapsed={compact}
              onClick={onCloseMobile}
            />
          );
        })}
      </div>

      {!compact && (
        <div className="mt-4 min-h-0 flex-1 overflow-y-auto">
          <div className="mb-1 flex items-center justify-between px-1">
            <span className="text-[11px] font-medium tracking-wide text-muted-dim uppercase">
              Chats
            </span>
            <div className="relative flex items-center">
              <IconButton
                label="Filter chats"
                size="sm"
                onClick={() => setFilterOpen((v) => !v)}
              >
                <FunnelSimple size={14} weight={filter === "newest" ? "regular" : "fill"} />
              </IconButton>
              <IconButton label="New folder" size="sm" onClick={() => setFolderDraft("")}>
                <FolderPlus size={14} />
              </IconButton>
              {filterOpen && (
                <div
                  className="absolute top-full right-8 z-20 mt-1 min-w-36 rounded-xl bg-raised p-1 shadow-[0_8px_24px_rgb(10_16_14/0.3)]"
                  role="menu"
                >
                  {FILTERS.map((f) => (
                    <button
                      key={f.id}
                      type="button"
                      role="menuitem"
                      className={cn(
                        "block w-full rounded-lg px-2 py-1 text-left text-xs hover:bg-raised-soft",
                        filter === f.id && "text-mint",
                      )}
                      onClick={() => {
                        setFilter(f.id);
                        setFilterOpen(false);
                      }}
                    >
                      {f.label}
                    </button>
                  ))}
                </div>
              )}
            </div>
          </div>

          {folderDraft !== null && (
            <form
              className="mb-1 px-1"
              onSubmit={(e) => {
                e.preventDefault();
                const title = folderDraft.trim() || "Untitled";
                setFolderDraft(null);
                void addFolder(title);
              }}
            >
              <input
                autoFocus
                value={folderDraft}
                onChange={(e) => setFolderDraft(e.target.value)}
                onBlur={() => {
                  if (!folderDraft.trim()) setFolderDraft(null);
                }}
                placeholder="Folder name"
                aria-label="Folder name"
                className="w-full rounded-lg bg-sidebar-hover px-2 py-1 text-sm outline-none"
              />
            </form>
          )}

          {folders.map((folder) => {
            const open = expanded[folder.id] !== false;
            const chats = byFolder.get(folder.id) || [];
            const folderDrop = dropTarget === `folder:${folder.id}`;
            return (
              <div
                key={folder.id}
                className="mb-1"
                onDragOver={(e) => {
                  e.preventDefault();
                  setDropTarget(`folder:${folder.id}`);
                }}
                onDrop={(e) => {
                  e.preventDefault();
                  void dropChat(folder.id, undefined, e);
                }}
              >
                <div
                  className={cn(
                    "flex items-center gap-0.5 rounded-lg",
                    folderDrop && "bg-raised-soft ring-1 ring-mint/40",
                  )}
                >
                  <button
                    type="button"
                    className="flex min-w-0 flex-1 items-center gap-1.5 rounded-lg px-2 py-1 text-left text-sm text-ink-soft hover:bg-sidebar-hover"
                    onClick={() => setExpanded((p) => ({ ...p, [folder.id]: !open }))}
                  >
                    {open ? <CaretDown size={12} /> : <CaretRight size={12} />}
                    <FolderSimple size={14} className="text-mint-dim" />
                    {renamingId === folder.id ? (
                      <input
                        autoFocus
                        value={renameValue}
                        aria-label="Rename folder"
                        className="min-w-0 flex-1 bg-transparent text-sm outline-none"
                        onClick={(e) => e.stopPropagation()}
                        onChange={(e) => setRenameValue(e.target.value)}
                        onBlur={() => void commitRename(folder.id, "folder")}
                        onKeyDown={(e) => {
                          if (e.key === "Enter") void commitRename(folder.id, "folder");
                        }}
                      />
                    ) : (
                      <span className="truncate">{folder.title}</span>
                    )}
                  </button>
                  <IconButton
                    label="Rename folder"
                    size="sm"
                    onClick={() => {
                      setRenamingId(folder.id);
                      setRenameValue(folder.title);
                    }}
                  >
                    <PencilSimple size={12} />
                  </IconButton>
                  <IconButton
                    label="Delete folder"
                    size="sm"
                    onClick={() => void removeFolder(folder.id)}
                  >
                    <Trash size={12} />
                  </IconButton>
                </div>
                {open && chats.map((c) => <ChatRow key={c.id} {...rowProps(c)} />)}
              </div>
            );
          })}

          <div
            className={cn(
              "min-h-8 rounded-lg",
              dropTarget === "unfiled" && "bg-raised-soft/80 ring-1 ring-mint/40",
            )}
            onDragOver={(e) => {
              e.preventDefault();
              setDropTarget("unfiled");
            }}
            onDrop={(e) => {
              e.preventDefault();
              void dropChat(null, undefined, e);
            }}
          >
            {unfiled.map((c) => (
              <ChatRow key={c.id} {...rowProps(c)} />
            ))}
          </div>
        </div>
      )}

      {compact && <div className="flex-1" />}

      <div className="mt-auto pt-2">
        <IconNavItem
          to="/settings"
          icon={<GearSix size={18} />}
          label="Settings"
          collapsed={compact}
          onClick={onCloseMobile}
        />
      </div>
    </aside>
  );
}

function ChatRow({
  title,
  time,
  active,
  renaming,
  renameValue,
  moving,
  folders,
  dragging,
  dropActive,
  onSelect,
  onRenameStart,
  onRenameChange,
  onRenameCommit,
  onDelete,
  onToggleMove,
  onMove,
  onDragStart,
  onDragEnd,
  onDragOver,
  onDrop,
}: {
  title: string;
  time: string;
  active: boolean;
  renaming: boolean;
  renameValue: string;
  moving: boolean;
  folders: { id: string; title: string }[];
  dragging: boolean;
  dropActive: boolean;
  onSelect: () => void;
  onRenameStart: () => void;
  onRenameChange: (v: string) => void;
  onRenameCommit: () => void;
  onDelete: () => void;
  onToggleMove: () => void;
  onMove: (folderId: string | null) => void;
  onDragStart: (e: DragEvent) => void;
  onDragEnd: () => void;
  onDragOver: (e: DragEvent) => void;
  onDrop: (e: DragEvent) => void;
}) {
  return (
    <div className="relative">
      <div
        draggable={!renaming}
        onDragStart={onDragStart}
        onDragEnd={onDragEnd}
        onDragOver={onDragOver}
        onDrop={onDrop}
        className={cn(
          "group flex cursor-grab items-center gap-1 rounded-xl px-2 py-1.5 active:cursor-grabbing",
          active ? "bg-raised-soft text-mint" : "text-ink-soft hover:bg-sidebar-hover",
          dragging && "opacity-40",
          dropActive && "ring-1 ring-mint/50",
        )}
      >
        <span
          className={cn("size-1.5 shrink-0 rounded-full", active ? "bg-mint" : "bg-muted-dim")}
        />
        {renaming ? (
          <input
            autoFocus
            value={renameValue}
            aria-label="Rename chat"
            className="min-w-0 flex-1 bg-transparent text-sm outline-none"
            onChange={(e) => onRenameChange(e.target.value)}
            onBlur={onRenameCommit}
            onKeyDown={(e) => {
              if (e.key === "Enter") onRenameCommit();
            }}
          />
        ) : (
          <button
            type="button"
            className="min-w-0 flex-1 truncate text-left text-sm"
            onClick={onSelect}
          >
            {title}
          </button>
        )}
        <span className="text-[10px] text-muted-dim group-hover:hidden">{time}</span>
        <div className="hidden group-hover:flex">
          <IconButton label="Rename chat" size="sm" onClick={onRenameStart}>
            <PencilSimple size={12} />
          </IconButton>
          <IconButton label="Move to folder" size="sm" onClick={onToggleMove}>
            <FolderSimple size={12} />
          </IconButton>
          <IconButton label="Delete chat" size="sm" onClick={onDelete}>
            <Trash size={12} />
          </IconButton>
        </div>
      </div>
      {moving && (
        <div className="absolute top-full right-1 z-10 mt-1 min-w-36 rounded-xl bg-raised p-1 shadow-[0_8px_24px_rgb(10_16_14/0.3)]">
          <button
            type="button"
            className="block w-full rounded-lg px-2 py-1 text-left text-xs hover:bg-raised-soft"
            onClick={() => onMove(null)}
          >
            Unfiled
          </button>
          {folders.map((f) => (
            <button
              key={f.id}
              type="button"
              className="block w-full rounded-lg px-2 py-1 text-left text-xs hover:bg-raised-soft"
              onClick={() => onMove(f.id)}
            >
              {f.title}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

export function MobileNav({ onOpenSidebar }: { onOpenSidebar: () => void }) {
  const { newChat } = useChatNav();
  const items = [
    { to: "/", label: "Today", icon: SquaresFour, end: true },
    { to: "/chat", label: "Chat", icon: ChatCircle },
    { to: "/goals", label: "Goals", icon: Target },
    { to: "/calendar", label: "Calendar", icon: CalendarBlank },
    { to: "/sparks", label: "Sparks", icon: Lightning },
  ];
  return (
    <nav
      className="fixed inset-x-0 bottom-0 z-20 flex items-center justify-around border-t border-hairline bg-sidebar px-1 py-1.5 md:hidden"
      aria-label="Mobile"
    >
      <button
        type="button"
        className="grid size-10 place-items-center text-muted"
        aria-label="Open menu"
        onClick={onOpenSidebar}
      >
        <SidebarSimple size={20} />
      </button>
      {items.map((item) => {
        const Icon = item.icon;
        return (
          <NavLink
            key={item.to}
            to={item.to}
            end={item.end}
            aria-label={item.label}
            className={({ isActive }) =>
              cn("grid size-10 place-items-center text-muted", isActive && "text-mint")
            }
          >
            <Icon size={22} />
          </NavLink>
        );
      })}
      <button
        type="button"
        className="grid size-10 place-items-center text-muted"
        aria-label="New chat"
        onClick={() => void newChat()}
      >
        <Plus size={20} />
      </button>
    </nav>
  );
}
