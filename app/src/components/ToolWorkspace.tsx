import type { ReactNode } from "react";

export function ToolWorkspace({
  sidebar,
  children,
  narrow,
}: {
  sidebar: ReactNode;
  children: ReactNode;
  narrow?: boolean;
}) {
  return (
    <div className="relative flex min-h-0 flex-1 overflow-hidden p-3">
      <aside
        className={`flex shrink-0 flex-col gap-2 overflow-y-auto border-r border-zinc-800 pr-2 [scrollbar-width:none] [&::-webkit-scrollbar]:w-0 ${
          narrow ? "w-28" : "w-40"
        }`}
      >
        {sidebar}
      </aside>
      <div className="flex min-h-0 min-w-0 flex-1 flex-col pl-3">{children}</div>
    </div>
  );
}
