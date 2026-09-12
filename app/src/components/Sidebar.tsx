import {
  Barbell,
  BookOpen,
  Brain,
  CaretDoubleLeft,
  CaretDoubleRight,
  CalendarBlank,
  CheckSquare,
  ChatsCircle,
  CircleNotch,
  Code,
  Cpu,
  DotsThree,
  FileText,
  Gear,
  Lightning,
  MagnifyingGlass,
  Play,
  Plus,
  ShareNetwork,
  SquaresFour,
  Trash,
  Wallet,
} from "@phosphor-icons/react";
import { useEffect, useState } from "react";
import { useAppStore } from "../stores/useAppStore";
import { useConversationStore } from "../stores/useConversationStore";
import { useChatStore } from "../stores/useChatStore";
import {
  FocusMode,
  useCodeAgentStore,
} from "../stores/useCodeAgentStore";
import { useSparkStore } from "../stores/useSparkStore";
import {
  createConversation,
  createCodexConversation,
  deleteConversation,
  loadCodexMessages,
  loadMessages,
  startRuntime,
} from "../lib/api";

function serviceLine(error: string | null, prefix: string): string | null {
  return error?.split("\n").find((line) => line.startsWith(prefix)) ?? null;
}

function StatusDot({
  icon,
  status,
  title,
  failed,
}: {
  icon: React.ReactNode;
  status: string;
  title: string;
  failed?: boolean;
}) {
  const online = status === "online";
  const checking = status === "checking";

  return (
    <div
      title={title}
      className="relative flex h-9 w-9 items-center justify-center text-zinc-500"
    >
      {icon}
      <span
        className={`absolute bottom-1.5 right-1.5 h-1.5 w-1.5 rounded-full ${
          checking
            ? "animate-pulse bg-amber-400"
            : online
              ? "bg-emerald-400"
              : failed
                ? "bg-rose-500"
                : "bg-zinc-600"
        }`}
      />
    </div>
  );
}

function RailButton({
  active,
  onClick,
  title,
  children,
}: {
  active?: boolean;
  onClick: () => void;
  title: string;
  children: React.ReactNode;
}) {
  return (
     <button
      type="button"
      onClick={onClick}
      title={title}
      className={`flex h-9 w-9 items-center justify-center rounded-xl transition ${
        active
          ? "bg-blue-500/15 text-blue-400"
          : "text-zinc-500 hover:bg-zinc-800 hover:text-zinc-300"
      }`}
    >
      {children}
    </button>
  );
}

export function Sidebar() {
  const { conversations } = useConversationStore();
  const {
    activeConversationId,
    setActiveConversationId,
    setMessages,
    isStreaming,
  } = useChatStore();
  const {
    currentPage,
    setCurrentPage,
    mlxStatus,
    brainStatus,
    runtimeStarting,
    runtimeError,
    sidebarCollapsed,
    toggleSidebar,
    setSidebarCollapsed,
  } = useAppStore();
  const { staleCount } = useSparkStore();
  const {
    setActiveConversationId: setCodeConversationId,
    setMessages: setCodeMessages,
    setFocus: setCodeFocus,
    setWorkspacePath: setCodeWorkspacePath,
    setPreviewUrl: setCodePreviewUrl,
    activeConversationId: activeCodeConversationId,
  } = useCodeAgentStore();
  const [deletingId, setDeletingId] = useState<string | null>(null);
  const [moreOpen, setMoreOpen] = useState(false);
  const [errorOpen, setErrorOpen] = useState(false);

  const brainError = serviceLine(runtimeError, "BRAIN_");
  const mlxError = serviceLine(runtimeError, "MLX_");

  useEffect(() => {
    setErrorOpen(Boolean(runtimeError));
  }, [runtimeError]);

  const isCodePage = currentPage === "code";
  const isLifePage =
    currentPage === "documents" ||
    currentPage === "fitness" ||
    currentPage === "money" ||
    currentPage === "study" ||
    currentPage === "todo" ||
    currentPage === "socials";
  const isBuddyChatSurface = currentPage === "chat" || isLifePage;

  function ensureBuddyConversation() {
    const active = useConversationStore
      .getState()
      .conversations.find((c) => c.id === activeConversationId);
    if (active?.kind === "research" || active?.kind === "codex") {
      const buddy = useConversationStore
        .getState()
        .conversations.find((c) => c.kind !== "research" && c.kind !== "codex");
      setActiveConversationId(buddy?.id ?? null);
      setMessages([]);
      if (buddy) loadMessages(buddy.id).catch(console.error);
    }
  }

  const visibleConversations = conversations.filter((c) => {
    if (isCodePage) return c.kind === "codex";
    return c.kind !== "codex";
  });

  async function handleNewChat() {
    const conv = await createConversation();
    setActiveConversationId(conv.id);
    setMessages([]);
    if (!isLifePage) setCurrentPage("chat");
    setSidebarCollapsed(false);
  }

  async function handleNewProject() {
    const conv = await createCodexConversation("New project", "planning");
    useConversationStore.getState().addConversation(conv);
    setCodeConversationId(conv.id);
    setCodeMessages([]);
    setCodeFocus("planning");
    setCodeWorkspacePath(conv.workspace_path ?? null);
    setCodePreviewUrl(null);
    setCurrentPage("code");
    setSidebarCollapsed(false);
  }

  function openCodeConversation(
    id: string,
    focus?: string | null,
    workspacePath?: string | null,
  ) {
    setCodeConversationId(id);
    setCodeFocus((focus as FocusMode) || "planning");
    setCodeWorkspacePath(workspacePath ?? null);
    setCodePreviewUrl(null);
    loadCodexMessages(id).catch(console.error);
    setCurrentPage("code");
  }

  async function handleDelete(id: string, e: React.MouseEvent) {
    e.stopPropagation();
    if (
      deletingId === id ||
      (isStreaming && activeConversationId === id)
    ) {
      return;
    }
    setDeletingId(id);
    try {
      await deleteConversation(id);
      if (activeConversationId === id) {
        setActiveConversationId(null);
        setMessages([]);
      }
    } catch (err) {
      console.error("delete failed:", err);
    } finally {
      setDeletingId(null);
    }
  }

  return (
    <div className="flex h-full shrink-0">
      {/* Icon rail */}
      <div className="flex w-[52px] shrink-0 flex-col items-center h-full">
        <img
          src="/app-icon.png"
          alt="Buddy"
          className="h-9 w-9 rounded-xl object-cover"
        />

        <nav className="mt-4 flex min-h-0 flex-1 flex-col items-center gap-1 overflow-y-auto">
          <RailButton
            active={currentPage === "dashboard"}
            onClick={() => setCurrentPage("dashboard")}
            title="Dashboard"
          >
            <SquaresFour
              size={20}
              weight={currentPage === "dashboard" ? "fill" : "regular"}
            />
          </RailButton>
          {sidebarCollapsed && (
            <RailButton onClick={toggleSidebar} title="Expand sidebar">
              <CaretDoubleRight size={18} />
            </RailButton>
          )}
          <div className="my-1 h-px w-6 bg-zinc-800" />
          <RailButton
            active={currentPage === "chat"}
            onClick={() => {
              ensureBuddyConversation();
              setCurrentPage("chat");
              if (sidebarCollapsed) setSidebarCollapsed(false);
            }}
            title="Chat"
          >
            <ChatsCircle
              size={20}
              weight={currentPage === "chat" ? "fill" : "regular"}
            />
          </RailButton>
          <RailButton
            active={currentPage === "calendar"}
            onClick={() => setCurrentPage("calendar")}
            title="Calendar"
          >
            <CalendarBlank
              size={20}
              weight={currentPage === "calendar" ? "fill" : "regular"}
            />
          </RailButton>
          <RailButton
            active={currentPage === "spark"}
            onClick={() => setCurrentPage("spark")}
            title="Sparks"
          >
            <span className="relative">
              <Lightning
                size={20}
                weight={currentPage === "spark" ? "fill" : "regular"}
              />
              {staleCount > 0 && (
                <span className="absolute -right-1 -top-1 flex h-3.5 min-w-3.5 items-center justify-center rounded-full bg-rose-500 px-0.5 text-[8px] font-bold text-white">
                  {staleCount > 9 ? "9+" : staleCount}
                </span>
              )}
            </span>
          </RailButton>
          <div className="relative">
            <RailButton
              active={moreOpen || isLifePage || isCodePage}
              onClick={() => setMoreOpen((open) => !open)}
              title="More"
            >
              <DotsThree size={22} weight={moreOpen || isLifePage || isCodePage ? "bold" : "regular"} />
            </RailButton>
            {moreOpen && (
              <div className="absolute left-12 top-0 z-20 w-44 rounded-xl border border-zinc-800 bg-zinc-900 p-1 shadow-xl">
                {[
                  { page: "documents" as const, label: "Documents", icon: FileText },
                  { page: "fitness" as const, label: "Fitness", icon: Barbell },
                  { page: "money" as const, label: "Money", icon: Wallet },
                  { page: "study" as const, label: "Study", icon: BookOpen },
                  { page: "todo" as const, label: "To-Do", icon: CheckSquare },
                  { page: "socials" as const, label: "Socials", icon: ShareNetwork },
                  { page: "code" as const, label: "Code", icon: Code },
                ].map((item) => (
                  <button
                    key={item.page}
                    type="button"
                    onClick={() => {
                      if (item.page === "code") {
                        setCurrentPage("code");
                        if (sidebarCollapsed) setSidebarCollapsed(false);
                      } else {
                        ensureBuddyConversation();
                        setCurrentPage(item.page);
                      }
                      setMoreOpen(false);
                    }}
                    className={`flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left text-sm ${
                      currentPage === item.page
                        ? "bg-blue-500/10 text-blue-400"
                        : "text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200"
                    }`}
                  >
                    <item.icon size={14} />
                    {item.label}
                  </button>
                ))}
              </div>
            )}
          </div>
        </nav>

        <div className="relative mt-auto flex shrink-0 flex-col items-center gap-1 pb-1 pt-1">
          <StatusDot
            icon={<Brain size={18} weight="duotone" />}
            status={brainStatus}
            failed={Boolean(brainError)}
            title={
              runtimeStarting
                ? "Brain starting…"
                : (brainError ?? `Brain ${brainStatus}`)
            }
          />
          <StatusDot
            icon={<Cpu size={18} weight="duotone" />}
            status={mlxStatus}
            failed={Boolean(mlxError)}
            title={
              runtimeStarting
                ? "MLX starting…"
                : (mlxError ?? `MLX ${mlxStatus}`)
            }
          />
          <button
            type="button"
            disabled={runtimeStarting}
            title={
              runtimeStarting
                ? "Starting Brain and MLX…"
                : runtimeError
                  ? runtimeError
                  : "Start Brain and MLX"
            }
            onClick={() => {
              startRuntime().catch(console.error);
            }}
            className={`flex h-9 w-9 items-center justify-center rounded-xl transition disabled:opacity-60 ${
              runtimeError
                ? "text-rose-400 hover:bg-rose-500/10 hover:text-rose-300"
                : "text-zinc-500 hover:bg-zinc-800 hover:text-zinc-300"
            }`}
          >
            {runtimeStarting ? (
              <CircleNotch size={18} className="animate-spin" />
            ) : (
              <Play size={16} weight="fill" />
            )}
          </button>
          {errorOpen && runtimeError && (
            <div className="absolute bottom-14 left-12 z-30 w-72 rounded-xl border border-rose-900/70 bg-zinc-950 p-2.5 shadow-xl">
              <div className="mb-1 flex items-center justify-between gap-2">
                <p className="text-[10px] font-medium uppercase tracking-wider text-rose-400">
                  Start failed
                </p>
                <button
                  type="button"
                  onClick={() => setErrorOpen(false)}
                  className="rounded px-1 text-[10px] text-zinc-500 hover:text-zinc-300"
                >
                  Dismiss
                </button>
              </div>
              <pre className="max-h-40 overflow-auto whitespace-pre-wrap break-words font-mono text-[11px] leading-relaxed text-zinc-300">
                {runtimeError}
              </pre>
            </div>
          )}
          <RailButton
            active={currentPage === "settings"}
            onClick={() => setCurrentPage("settings")}
            title="Settings"
          >
            <Gear
              size={20}
              weight={currentPage === "settings" ? "fill" : "regular"}
            />
          </RailButton>
        </div>
      </div>

      {/* Expandable panel */}
      <div
        className={`flex h-full shrink-0 overflow-hidden transition-[width,opacity,margin] duration-300 ease-in-out ${
          sidebarCollapsed
            ? "pointer-events-none ml-0 w-0 opacity-0"
            : "ml-2 w-56 opacity-100"
        }`}
        aria-hidden={sidebarCollapsed}
      >
        <div
          className={`flex w-56 shrink-0 flex-col rounded-2xl bg-zinc-900 ${
            !sidebarCollapsed ? "sidebar-panel-inner" : ""
          }`}
        >
          <div className="grid grid-cols-3 items-center px-2 py-3">
            <button
              type="button"
              onClick={toggleSidebar}
              title="Close"
              className="flex h-7 w-7 items-center justify-center rounded-lg text-zinc-500 transition hover:bg-zinc-800 hover:text-zinc-300"
            >
              <CaretDoubleLeft size={16} />
            </button>
            <p className="text-center text-[10px] font-medium tracking-wider text-zinc-500">
              {isCodePage ? "PROJECTS" : "CHATS"}
            </p>
            <button
              type="button"
              onClick={isCodePage ? handleNewProject : handleNewChat}
              className="flex h-7 w-7 items-center justify-center justify-self-end rounded-lg text-zinc-500 transition hover:bg-zinc-800 hover:text-zinc-300"
              title={isCodePage ? "New project" : "New chat"}
            >
              <Plus size={14} weight="bold" />
            </button>
          </div>

          <div className="flex-1 overflow-y-auto px-3 pb-3">
            {visibleConversations.map((conv) => {
              const isActive = isCodePage
                ? activeCodeConversationId === conv.id
                : activeConversationId === conv.id && isBuddyChatSurface;
              const deleteBlocked =
                !isCodePage && isStreaming && activeConversationId === conv.id;
              const showDelete =
                deletingId === conv.id || isActive;
              const accent = isCodePage
                ? "bg-violet-500/10 font-medium text-violet-400"
                : "bg-blue-500/10 font-medium text-blue-400";

              return (
                <div
                  key={conv.id}
                  className={`group mb-0.5 flex w-full items-center rounded-lg text-sm transition ${
                    isActive
                      ? accent
                      : "text-zinc-400 hover:bg-zinc-800/60 hover:text-zinc-200"
                  }`}
                >
                  <button
                    type="button"
                    onClick={() => {
                      if (isCodePage) {
                        openCodeConversation(
                          conv.id,
                          conv.focus_mode,
                          conv.workspace_path,
                        );
                      } else {
                        setActiveConversationId(conv.id);
                        if (conv.kind === "research" || !isLifePage) setCurrentPage("chat");
                      }
                    }}
                    className="flex min-w-0 flex-1 items-center gap-1.5 truncate px-2.5 py-2 text-left"
                  >
                    {conv.kind === "research" && (
                      <MagnifyingGlass size={12} className="shrink-0 text-zinc-500" />
                    )}
                    <span className="truncate">{conv.title}</span>
                  </button>
                  <button
                    type="button"
                    onClick={(e) => handleDelete(conv.id, e)}
                    disabled={deleteBlocked || deletingId === conv.id}
                    title={
                      deleteBlocked
                        ? "Wait for the reply to finish"
                        : "Delete chat"
                    }
                    className={`mr-1 shrink-0 rounded p-1 transition ${
                      showDelete ? "inline-flex" : "hidden group-hover:inline-flex"
                    } ${
                      deletingId === conv.id
                        ? "text-zinc-500"
                        : deleteBlocked
                          ? "cursor-not-allowed text-zinc-600"
                          : "text-zinc-500 hover:bg-zinc-700 hover:text-rose-400"
                    }`}
                  >
                    {deletingId === conv.id ? (
                      <CircleNotch size={13} className="animate-spin" />
                    ) : (
                      <Trash size={13} />
                    )}
                  </button>
                </div>
              );
            })}
          </div>
        </div>
      </div>

    </div>
  );
}
