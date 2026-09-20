import { List } from "@phosphor-icons/react";
import { useState } from "react";
import { Outlet } from "react-router-dom";
import { ChatNavProvider } from "./chatNav";
import { MobileNav, SharedSidebar } from "./components/SharedSidebar";
import { IconButton } from "./components/ui/IconButton";
import { ToastProvider } from "./components/ui/Toast";

export function AppShell() {
  const [collapsed, setCollapsed] = useState(false);
  const [mobileOpen, setMobileOpen] = useState(false);

  return (
    <ToastProvider>
      <ChatNavProvider>
        <div className="flex h-dvh bg-page font-ui text-ink">
          <SharedSidebar
            collapsed={collapsed}
            mobileOpen={mobileOpen}
            onToggleCollapsed={() => setCollapsed((v) => !v)}
            onCloseMobile={() => setMobileOpen(false)}
          />
          <div className="flex min-h-0 min-w-0 flex-1 flex-col">
            <div className="flex items-center gap-2 px-3 pt-3 md:hidden">
              <IconButton label="Open menu" onClick={() => setMobileOpen(true)}>
                <List size={20} />
              </IconButton>
            </div>
            <main className="min-h-0 min-w-0 flex-1 overflow-hidden pb-16 md:pb-0">
              <Outlet />
            </main>
          </div>
          <MobileNav onOpenSidebar={() => setMobileOpen(true)} />
        </div>
      </ChatNavProvider>
    </ToastProvider>
  );
}
