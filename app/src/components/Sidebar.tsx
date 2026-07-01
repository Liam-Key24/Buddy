import {
  Brain,
  CaretDoubleLeft,
  CaretDoubleRight,
  ChatsCircle,
  CircleNotch,
  Cpu,
  Gear,
  Plus,
  SquaresFour,
  Trash,
} from "@phosphor-icons/react";
import { useState } from "react";
import { useAppStore } from "../stores/useAppStore";
import { useConversationStore } from "../stores/useConversationStore";
import { useChatStore } from "../stores/useChatStore";
import { createConversation, deleteConversation } from "../lib/api";

function StatusIcon({
  icon,
  status,
  title,
}: {
  icon: React.ReactNode;
  status: string;
  title: string;
}) {
  const online = status === "online";
  const checking = status === "checking";

  return (
    <button
      type="button"
      title={title}
      className="relative flex h-9 w-9 items-center justify-center rounded-xl text-zinc-500 transition hover:bg-zinc-800 hover:text-zinc-300"
    >
      {icon}
      <span
        className={`absolute bottom-1.5 right-1.5 h-1.5 w-1.5 rounded-full ${
          checking
            ? "animate-pulse bg-amber-400"
            : online
              ? "bg-emerald-400"
              : "bg-zinc-600"
        }`}
      />
    </button>
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
    sidebarCollapsed,
    toggleSidebar,
    setSidebarCollapsed,
  } = useAppStore();
  const [deletingId, setDeletingId] = useState<string | null>(null);

  async function handleNewChat() {
    const conv = await createConversation();
    setActiveConversationId(conv.id);
    setMessages([]);
    setCurrentPage("chat");
    setSidebarCollapsed(false);
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

        <nav className="mt-4 flex flex-col items-center gap-1">
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
        </nav>

        <div className="mt-auto flex flex-col items-center gap-1 pb-1">
          <StatusIcon
            icon={<Cpu size={18} weight="duotone" />}
            status={mlxStatus}
            title={`MLX ${mlxStatus}`}
          />
          <StatusIcon
            icon={<Brain size={18} weight="duotone" />}
            status={brainStatus}
            title={`Brain ${brainStatus}`}
          />
          <RailButton
            active={currentPage === "chat"}
            onClick={() => {
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
              CHATS
            </p>
            <button
              type="button"
              onClick={handleNewChat}
              className="flex h-7 w-7 items-center justify-center justify-self-end rounded-lg text-zinc-500 transition hover:bg-zinc-800 hover:text-zinc-300"
              title="New chat"
            >
              <Plus size={14} weight="bold" />
            </button>
          </div>

          <div className="flex-1 overflow-y-auto px-3 pb-3">
            {conversations.map((conv) => {
              const isActive =
                activeConversationId === conv.id && currentPage === "chat";
              const deleteBlocked =
                isStreaming && activeConversationId === conv.id;
              const showDelete =
                deletingId === conv.id || isActive;

              return (
                <div
                  key={conv.id}
                  className={`group mb-0.5 flex w-full items-center rounded-lg text-sm transition ${
                    isActive
                      ? "bg-blue-500/10 font-medium text-blue-400"
                      : "text-zinc-400 hover:bg-zinc-800/60 hover:text-zinc-200"
                  }`}
                >
                  <button
                    type="button"
                    onClick={() => {
                      setActiveConversationId(conv.id);
                      setCurrentPage("chat");
                    }}
                    className="min-w-0 flex-1 truncate px-2.5 py-2 text-left"
                  >
                    {conv.title}
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
