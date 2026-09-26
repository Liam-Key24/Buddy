import { useEffect, useState } from "react";
import { Outlet } from "react-router-dom";
import { fetchSettings } from "./api";
import { ChatNavProvider } from "./chatNav";
import { MobileTopBar, SharedSidebar } from "./components/SharedSidebar";
import { GoalCompleteProvider } from "./components/ui/GoalCompleteOverlay";
import { ToastProvider } from "./components/ui/Toast";

export function AppShell() {
  const [collapsed, setCollapsed] = useState(false);
  const [mobileOpen, setMobileOpen] = useState(false);

  useEffect(() => {
    let cancelled = false;
    fetchSettings()
      .then((s) => {
        if (!cancelled && s.compact_sidebar) setCollapsed(true);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <ToastProvider>
      <GoalCompleteProvider>
        <ChatNavProvider>
          <div className="flex h-dvh bg-page font-ui text-ink">
            <SharedSidebar
              collapsed={collapsed}
              mobileOpen={mobileOpen}
              onToggleCollapsed={() => setCollapsed((v) => !v)}
              onCloseMobile={() => setMobileOpen(false)}
            />
            <div className="flex min-h-0 min-w-0 flex-1 flex-col">
              <MobileTopBar onOpenSidebar={() => setMobileOpen(true)} />
              <main className="min-h-0 min-w-0 flex-1 overflow-hidden pt-16 lg:pt-0">
                <Outlet />
              </main>
            </div>
          </div>
        </ChatNavProvider>
      </GoalCompleteProvider>
    </ToastProvider>
  );
}
