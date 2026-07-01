import { useAppStore } from "../stores/useAppStore";
import { useConversationStore } from "../stores/useConversationStore";
import { useChatStore } from "../stores/useChatStore";
import { createConversation, deleteConversation } from "../lib/api";

export function Sidebar() {
  const { conversations } = useConversationStore();
  const { activeConversationId, setActiveConversationId, setMessages } =
    useChatStore();
  const { currentPage, setCurrentPage } = useAppStore();

  async function handleNewChat() {
    const conv = await createConversation();
    setActiveConversationId(conv.id);
    setMessages([]);
    setCurrentPage("chat");
  }

  async function handleDelete(id: string, e: React.MouseEvent) {
    e.stopPropagation();
    await deleteConversation(id);
    if (activeConversationId === id) {
      setActiveConversationId(null);
      setMessages([]);
    }
  }

  return (
    <aside className="flex w-64 shrink-0 flex-col border-r border-gray-800 bg-[#0a0c10]">
      <div className="p-3">
        <button
          onClick={handleNewChat}
          className="w-full rounded-lg bg-indigo-600 px-3 py-2 text-sm font-medium text-white transition hover:bg-indigo-500"
        >
          New chat
        </button>
      </div>

      <nav className="flex-1 overflow-y-auto px-2">
        {conversations.map((conv) => (
          <button
            key={conv.id}
            onClick={() => {
              setActiveConversationId(conv.id);
              setCurrentPage("chat");
            }}
            className={`group mb-1 flex w-full items-center justify-between rounded-lg px-3 py-2 text-left text-sm transition ${
              activeConversationId === conv.id
                ? "bg-gray-800 text-white"
                : "text-gray-400 hover:bg-gray-800/50 hover:text-gray-200"
            }`}
          >
            <span className="truncate">{conv.title}</span>
            <span
              onClick={(e) => handleDelete(conv.id, e)}
              className="ml-2 hidden shrink-0 text-gray-500 hover:text-red-400 group-hover:inline"
            >
              ×
            </span>
          </button>
        ))}
      </nav>

      <div className="border-t border-gray-800 p-2">
        <button
          onClick={() => setCurrentPage("settings")}
          className={`w-full rounded-lg px-3 py-2 text-left text-sm transition ${
            currentPage === "settings"
              ? "bg-gray-800 text-white"
              : "text-gray-400 hover:bg-gray-800/50"
          }`}
        >
          Settings
        </button>
      </div>
    </aside>
  );
}
