import { useEffect } from "react";
import { Header } from "./components/Header";
import { Sidebar } from "./components/Sidebar";
import { ChatWindow } from "./components/ChatWindow";
import { ChatInput } from "./components/ChatInput";
import { Settings } from "./pages/Settings";
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
    if (activeConversationId) {
      loadMessages(activeConversationId);
    }
  }, [activeConversationId]);

  return (
    <div className="flex h-screen overflow-hidden bg-[#0f1117]">
      <Sidebar />
      <div className="flex flex-1 flex-col">
        <Header />
        {currentPage === "settings" ? (
          <Settings />
        ) : (
          <>
            <ChatWindow />
            <ChatInput />
          </>
        )}
      </div>
    </div>
  );
}

export default App;
