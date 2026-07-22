import { useEffect } from "react";
import { isPermissionGranted, requestPermission } from "@tauri-apps/plugin-notification";
import { Header } from "./components/Header";
import { Sidebar } from "./components/Sidebar";
import { ChatWindow } from "./components/ChatWindow";
import { ChatInput } from "./components/ChatInput";
import { PageTransition } from "./components/PageTransition";
import { Settings } from "./pages/Settings";
import { Dashboard } from "./pages/Dashboard";
import { Spark } from "./pages/Spark";
import { CodeAgent } from "./pages/CodeAgent";
import { useAppStore } from "./stores/useAppStore";
import { useChatStore } from "./stores/useChatStore";
import { useCodeAgentStore } from "./stores/useCodeAgentStore";
import { useSparkStore } from "./stores/useSparkStore";
import {
  fetchServiceStatus,
  loadCodexMessages,
  loadConversations,
  loadMessages,
  startBrain,
  subscribeCodexEvents,
  subscribeSparkEvents,
} from "./lib/api";

function App() {
  const { currentPage, setMlxStatus, setBrainStatus, setCurrentPage } =
    useAppStore();
  const { activeConversationId } = useChatStore();
  const { refresh, refreshStale } = useSparkStore();

  useEffect(() => {
    loadConversations();
    startBrain().catch(console.error);
    refresh().catch(console.error);

    isPermissionGranted()
      .then((granted) => {
        if (!granted) return requestPermission();
      })
      .catch(console.error);

    const unsub = subscribeSparkEvents(
      (count) => useSparkStore.setState({ staleCount: count }),
      () => {
        refresh().catch(console.error);
      },
      () => setCurrentPage("spark"),
    );

    const unsubCodex = subscribeCodexEvents(
      (chunk) => useCodeAgentStore.getState().appendStreaming(chunk),
      () => {
        const convId = useCodeAgentStore.getState().activeConversationId;
        if (convId) {
          loadCodexMessages(convId).catch(console.error);
        }
        useCodeAgentStore.getState().clearStreaming();
      },
      (message) => console.error("codex error:", message),
      (url) => useCodeAgentStore.getState().setPreviewUrl(url),
    );

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
    const staleInterval = setInterval(() => refreshStale().catch(console.error), 60 * 60 * 1000);
    return () => {
      clearInterval(interval);
      clearInterval(staleInterval);
      unsub();
      unsubCodex();
    };
  }, [setMlxStatus, setBrainStatus, refresh, refreshStale, setCurrentPage]);

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
            ) : page === "spark" ? (
              <Spark />
            ) : page === "code" ? (
              <CodeAgent />
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
