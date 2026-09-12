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
import { Calendar } from "./pages/Calendar";
import { Documents } from "./pages/Documents";
import { Fitness } from "./pages/Fitness";
import { Money } from "./pages/Money";
import { Study } from "./pages/Study";
import { TodoPage } from "./pages/Todo";
import { Socials } from "./pages/Socials";
import { useAppStore } from "./stores/useAppStore";
import { pickWorkspacePage } from "./lib/workspaceFocus";
import { useChatStore } from "./stores/useChatStore";
import { useCodeAgentStore } from "./stores/useCodeAgentStore";
import { useSparkStore } from "./stores/useSparkStore";
import { useCalendarStore } from "./stores/useCalendarStore";
import { useCalendarNotificationStore } from "./stores/useCalendarNotificationStore";
import { useLifestyleStore } from "./stores/useLifestyleStore";
import { useTodoStore } from "./stores/useTodoStore";
import {
  fetchServiceStatus,
  loadCodexMessages,
  loadConversations,
  loadMessages,
  subscribeCalendarEvents,
  subscribeCalendarProposal,
  subscribeCalendarReminders,
  subscribeCodexEvents,
  subscribeSparkEvents,
  subscribeWorkspaceFocus,
} from "./lib/api";
import { subscribeLifeEvent } from "./lib/lifeApi";

function App() {
  const { currentPage, setMlxStatus, setBrainStatus, setCurrentPage } =
    useAppStore();
  const { activeConversationId } = useChatStore();
  const { refresh, refreshStale } = useSparkStore();

  useEffect(() => {
    loadConversations();
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

    let unsubCalendar = () => {};
    let unsubReminders = () => {};
    let unsubProposal = () => {};
    subscribeCalendarEvents(() => {
      useCalendarStore.getState().loadRange().catch(console.error);
      useLifestyleStore.getState().loadBlocks().catch(console.error);
      useLifestyleStore.getState().loadRules().catch(console.error);
    }).then((unsub) => {
      unsubCalendar = unsub;
    });
    subscribeCalendarProposal((payload) => {
      if (payload.cleared || !payload.blocks?.length) {
        useCalendarStore.getState().clearProposal();
        return;
      }
      useCalendarStore
        .getState()
        .setProposal(payload.blocks, payload.conversation_id ?? null);
    }).then((unsub) => {
      unsubProposal = unsub;
    });
    subscribeCalendarReminders(
      (delivery) => {
        useCalendarNotificationStore.getState().pushReminder(delivery);
      },
      (count) => {
        useCalendarNotificationStore.getState().setCount(count);
      },
    ).then((unsub) => {
      unsubReminders = unsub;
    });
    useCalendarNotificationStore.getState().refresh().catch(console.error);

    const unsubTodos = subscribeLifeEvent("todos-updated", () => {
      useTodoStore.getState().refresh().catch(console.error);
    });
    const unsubSocials = subscribeLifeEvent("socials-updated", () => {
      useCalendarStore.getState().loadRange().catch(console.error);
    });
    let unsubWorkspace = () => {};
    subscribeWorkspaceFocus((payload) => {
      const store = useAppStore.getState();
      if (payload.focus_doc) {
        store.setPendingWorkspaceDocId(payload.focus_doc);
      }
      const next = pickWorkspacePage(payload.pages ?? [], store.currentPage);
      if (next) store.setCurrentPage(next);
    }).then((unsub) => {
      unsubWorkspace = unsub;
    });

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
      unsubCalendar();
      unsubReminders();
      unsubProposal();
      unsubTodos();
      unsubSocials();
      unsubWorkspace();
    };
  }, [setMlxStatus, setBrainStatus, refresh, refreshStale, setCurrentPage]);

  useEffect(() => {
    if (activeConversationId && !useChatStore.getState().isStreaming) {
      loadMessages(activeConversationId);
    }
  }, [activeConversationId]);

  useEffect(() => {
    const mq = window.matchMedia("(max-width: 1199px)");
    const apply = () => {
      if (mq.matches) useAppStore.getState().setSidebarCollapsed(true);
    };
    apply();
    mq.addEventListener("change", apply);
    return () => mq.removeEventListener("change", apply);
  }, []);

  return (
    <div className="h-screen overflow-x-auto overflow-y-hidden bg-zinc-950">
      <div className="flex h-full min-w-[1080px] gap-2 p-2">
      <Sidebar />
      <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden rounded-2xl bg-zinc-900">
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
            ) :             page === "calendar" ? (
              <Calendar />
            ) : page === "documents" ? (
              <Documents />
            ) : page === "fitness" ? (
              <Fitness />
            ) : page === "money" ? (
              <Money />
            ) : page === "study" ? (
              <Study />
            ) : page === "todo" ? (
              <TodoPage />
            ) : page === "socials" ? (
              <Socials />
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
    </div>
  );
}

export default App;
