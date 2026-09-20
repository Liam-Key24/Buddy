import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { useNavigate } from "react-router-dom";
import {
  createConversation,
  createFolder,
  deleteConversation,
  deleteFolder,
  fetchFolders,
  listConversations,
  moveConversation,
  renameConversation,
  renameFolder,
  restoreConversation,
  type ChatFolder,
  type Conversation,
} from "./api";
import { useToast } from "./components/ui/Toast";

export const STORAGE_KEY = "buddy.conversationId";

type ChatNavValue = {
  conversations: Conversation[];
  folders: ChatFolder[];
  conversationId: string | null;
  search: string;
  setSearch: (value: string) => void;
  selectConversation: (id: string, opts?: { navigate?: boolean }) => void;
  setConversationId: (id: string | null) => void;
  newChat: () => Promise<Conversation>;
  renameChat: (id: string, title: string) => Promise<void>;
  deleteChat: (id: string) => Promise<void>;
  restoreChat: (id: string) => Promise<void>;
  moveChat: (id: string, folderId: string | null) => Promise<void>;
  addFolder: (title: string) => Promise<ChatFolder>;
  renameFolderTitle: (id: string, title: string) => Promise<void>;
  removeFolder: (id: string) => Promise<void>;
  refresh: () => Promise<void>;
};

const ChatNavContext = createContext<ChatNavValue | null>(null);

export function useChatNav() {
  const ctx = useContext(ChatNavContext);
  if (!ctx) throw new Error("useChatNav must be used within ChatNavProvider");
  return ctx;
}

export function ChatNavProvider({ children }: { children: ReactNode }) {
  const navigate = useNavigate();
  const { pushToast } = useToast();
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [folders, setFolders] = useState<ChatFolder[]>([]);
  const [conversationId, setConversationIdState] = useState<string | null>(() =>
    localStorage.getItem(STORAGE_KEY),
  );
  const [search, setSearch] = useState("");

  const refresh = useCallback(async () => {
    const [chats, folderRows] = await Promise.all([
      listConversations(),
      fetchFolders().catch(() => [] as ChatFolder[]),
    ]);
    setConversations(chats);
    setFolders(folderRows);
  }, []);

  useEffect(() => {
    refresh().catch(() => undefined);
  }, [refresh]);

  const setConversationId = useCallback((id: string | null) => {
    setConversationIdState(id);
    if (id) localStorage.setItem(STORAGE_KEY, id);
    else localStorage.removeItem(STORAGE_KEY);
  }, []);

  const selectConversation = useCallback(
    (id: string, opts?: { navigate?: boolean }) => {
      setConversationId(id);
      if (opts?.navigate !== false) navigate("/chat");
    },
    [navigate, setConversationId],
  );

  const newChat = useCallback(async () => {
    const created = await createConversation();
    await refresh();
    setConversationId(created.id);
    navigate("/chat");
    return created;
  }, [navigate, refresh, setConversationId]);

  const renameChat = useCallback(
    async (id: string, title: string) => {
      await renameConversation(id, title);
      await refresh();
    },
    [refresh],
  );

  const deleteChat = useCallback(
    async (id: string) => {
      await deleteConversation(id);
      await refresh();
      if (conversationId === id) {
        const next = conversations.find((c) => c.id !== id);
        if (next) setConversationId(next.id);
        else setConversationId(null);
      }
      pushToast("Moved to Recently Deleted.", {
        label: "Undo",
        onClick: () => {
          restoreConversation(id)
            .then(() => refresh())
            .then(() => {
              setConversationId(id);
              pushToast("Chat restored");
            })
            .catch(() => undefined);
        },
      });
    },
    [conversationId, conversations, pushToast, refresh, setConversationId],
  );

  const restoreChat = useCallback(
    async (id: string) => {
      await restoreConversation(id);
      await refresh();
    },
    [refresh],
  );

  const moveChat = useCallback(
    async (id: string, folderId: string | null) => {
      await moveConversation(id, folderId);
      await refresh();
    },
    [refresh],
  );

  const addFolder = useCallback(
    async (title: string) => {
      const row = await createFolder(title);
      await refresh();
      return row;
    },
    [refresh],
  );

  const renameFolderTitle = useCallback(
    async (id: string, title: string) => {
      await renameFolder(id, title);
      await refresh();
    },
    [refresh],
  );

  const removeFolder = useCallback(
    async (id: string) => {
      await deleteFolder(id);
      await refresh();
      pushToast("Folder removed. Chats were unfiled.");
    },
    [pushToast, refresh],
  );

  const value = useMemo(
    () => ({
      conversations,
      folders,
      conversationId,
      search,
      setSearch,
      selectConversation,
      setConversationId,
      newChat,
      renameChat,
      deleteChat,
      restoreChat,
      moveChat,
      addFolder,
      renameFolderTitle,
      removeFolder,
      refresh,
    }),
    [
      conversations,
      folders,
      conversationId,
      search,
      selectConversation,
      setConversationId,
      newChat,
      renameChat,
      deleteChat,
      restoreChat,
      moveChat,
      addFolder,
      renameFolderTitle,
      removeFolder,
      refresh,
    ],
  );

  return <ChatNavContext.Provider value={value}>{children}</ChatNavContext.Provider>;
}
