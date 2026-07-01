import { useEffect } from "react";
import { Header } from "./components/Header";
import { Sidebar } from "./components/Sidebar";
import { ChatWindow } from "./components/ChatWindow";
import { ChatInput } from "./components/ChatInput";
import { PageTransition } from "./components/PageTransition";
import { Settings } from "./pages/Settings";
import { Dashboard } from "./pages/Dashboard";
import { useAppStore } from "./stores/useAppStore";
import { useChatStore } from "./stores/useChatStore";
import {
  fetchServiceStatus,
  loadConversations,
  loadMessages,
  startBrain,
} from "./lib/api";

function App() {
  const { currentPage, setMlxStatus, setBrainStatus } = useAppStore();
  const { activeConversationId } = useChatStore();

  useEffect(() => {
    loadConversations();
    startBrain().catch(console.error);

    async function pollStatus() {
      try {
        const status = await fetchServiceStatus();
        setMlxStatus(status.mlx ? "online" : "offline");
        setBrainStatus(status.brain ? "online" : "offline");
      } catch {
        setMlxStatus("offline");
        setBrainStatus("offline");
      }
    }

    pollStatus();
    const interval = setInterval(pollStatus, 5000);
    return () => clearInterval(interval);
  }, [setMlxStatus, setBrainStatus]);

  useEffect(() => {
    if (activeConversationId && !useChatStore.getState().isStreaming) {
      loadMessages(activeConversationId);
    }
  }, [activeConversationId]);

  return (
    <div className="flex h-screen overflow-hidden bg-zinc-950 p-2 gap-2">
      <Sidebar />
      <div className="flex min-w-0 flex-1 flex-col overflow-hidden rounded-2xl bg-zinc-900">
        <Header />
        <PageTransition
          page={currentPage}
          render={(page) =>
            page === "settings" ? (
              <Settings />
            ) : page === "dashboard" ? (
              <Dashboard />
            ) : (
              <>
                <ChatWindow />
                <ChatInput />
              </>
            )
          }
        />
      </div>
    </div>
  );
}

export default App;
