import { useState } from "react";
import { Terminal as TerminalIcon, Globe } from "@phosphor-icons/react";
import { CodeAgentWindow } from "../components/CodeAgentWindow";
import { CodeAgentInput } from "../components/CodeAgentInput";
import { CodeTerminal } from "../components/CodeTerminal";
import { CodePreview } from "../components/CodePreview";
import { useCodeAgentStore } from "../stores/useCodeAgentStore";

type Panel = "terminal" | "preview";

export function CodeAgent() {
  const [panel, setPanel] = useState<Panel>("terminal");
  const { workspacePath } = useCodeAgentStore();

  return (
    <div className="flex flex-1 overflow-hidden">
      {/* Chat column */}
      <div className="flex min-w-0 flex-1 flex-col border-r border-zinc-800">
        <CodeAgentWindow />
        <CodeAgentInput />
      </div>

      {/* Tools column: terminal + live preview */}
      <div className="flex w-[46%] min-w-[320px] flex-col">
        <div className="flex items-center gap-1 border-b border-zinc-800 px-2 py-1.5">
          <PanelTab
            active={panel === "terminal"}
            onClick={() => setPanel("terminal")}
            icon={<TerminalIcon size={13} weight="bold" />}
            label="Terminal"
          />
          <PanelTab
            active={panel === "preview"}
            onClick={() => setPanel("preview")}
            icon={<Globe size={13} weight="bold" />}
            label="Preview"
          />
        </div>
        <div className="min-h-0 flex-1">
          {/* Keep terminal mounted so its session persists across tab switches */}
          <div className={panel === "terminal" ? "h-full" : "hidden"}>
            <CodeTerminal cwd={workspacePath} />
          </div>
          <div className={panel === "preview" ? "h-full" : "hidden"}>
            <CodePreview />
          </div>
        </div>
      </div>
    </div>
  );
}

function PanelTab({
  active,
  onClick,
  icon,
  label,
}: {
  active: boolean;
  onClick: () => void;
  icon: React.ReactNode;
  label: string;
}) {
  return (
    <button
      onClick={onClick}
      className={`flex items-center gap-1.5 rounded-lg px-2.5 py-1 text-xs font-medium transition ${
        active
          ? "bg-violet-500/15 text-violet-400"
          : "text-zinc-500 hover:bg-zinc-800 hover:text-zinc-300"
      }`}
    >
      {icon}
      {label}
    </button>
  );
}
