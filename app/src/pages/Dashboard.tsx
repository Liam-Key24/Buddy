import {
  Brain,
  ChatsCircle,
  Cpu,
  Sparkle,
} from "@phosphor-icons/react";
import { useConversationStore } from "../stores/useConversationStore";
import { useAppStore } from "../stores/useAppStore";
import { useChatStore } from "../stores/useChatStore";
import { useSettingsStore } from "../stores/useSettingsStore";
import { loadSettings } from "../lib/api";
import { useEffect } from "react";

function formatDate(ts: number) {
  return new Date(ts * 1000).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
  });
}

function shortModel(name: string) {
  const basename = name.split("/").pop() ?? name;
  const [family, versionPart] = basename.split("-");
  if (!family) return basename.toLowerCase();
  const major = versionPart?.match(/^(\d+)/)?.[1];
  return major ? `${family.toLowerCase()}-${major}` : family.toLowerCase();
}

export function Dashboard() {
  const { conversations } = useConversationStore();
  const { mlxStatus, brainStatus, setCurrentPage } = useAppStore();
  const { setActiveConversationId } = useChatStore();
  const settings = useSettingsStore();

  useEffect(() => {
    loadSettings().then((s) => {
      settings.setSettings({
        mlxUrl: s.mlx_url,
        brainUrl: s.brain_url,
        modelName: s.model_name,
        logLevel: s.log_level,
        autoStartMlx: s.auto_start_mlx,
      });
    });
  }, []);

  const recent = [...conversations]
    .sort((a, b) => b.updated_at - a.updated_at)
    .slice(0, 5);

  function openChat(id: string) {
    setActiveConversationId(id);
    setCurrentPage("chat");
  }

  return (
    <div className="flex-1 overflow-y-auto p-6">
      <div className="mx-auto max-w-5xl space-y-6">
        <div className="dashboard-stagger grid grid-cols-2 gap-4 sm:grid-cols-4">
          <StatCard
            icon={<ChatsCircle size={22} weight="duotone" />}
            value={String(conversations.length)}
            label="Chats"
            bg="bg-sky-950"
            iconColor="text-sky-400"
          />
          <StatCard
            icon={<Cpu size={22} weight="duotone" />}
            value={mlxStatus === "online" ? "On" : "Off"}
            label="MLX"
            bg="bg-emerald-950"
            iconColor={mlxStatus === "online" ? "text-emerald-400" : "text-zinc-500"}
          />
          <StatCard
            icon={<Brain size={22} weight="duotone" />}
            value={brainStatus === "online" ? "On" : "Off"}
            label="Brain"
            bg="bg-amber-950"
            iconColor={brainStatus === "online" ? "text-amber-400" : "text-zinc-500"}
          />
          <StatCard
            icon={<Sparkle size={22} weight="duotone" />}
            value={shortModel(settings.modelName)}
            label="Model"
            bg="bg-blue-950"
            iconColor="text-blue-400"
            small
          />
        </div>

        <section className="dashboard-panel rounded-2xl border border-zinc-800 bg-zinc-900 p-5 shadow-sm">
          <h2 className="text-sm font-medium text-zinc-400">Recent</h2>
          <div className="mt-3 space-y-2">
            {recent.length === 0 ? (
              <p className="py-8 text-center text-sm text-zinc-500">—</p>
            ) : (
              recent.map((conv) => (
                <button
                  key={conv.id}
                  onClick={() => openChat(conv.id)}
                  className="flex w-full items-center justify-between rounded-xl border border-zinc-800 bg-zinc-800/50 px-4 py-3 text-left transition hover:border-zinc-700 hover:bg-zinc-800"
                >
                  <div className="min-w-0">
                    <p className="truncate text-sm font-medium text-zinc-200">
                      {conv.title}
                    </p>
                    <p className="mt-0.5 text-xs text-zinc-500">
                      {formatDate(conv.updated_at)}
                    </p>
                  </div>
                  <span className="ml-3 h-2 w-2 shrink-0 rounded-full bg-emerald-400" />
                </button>
              ))
            )}
          </div>
        </section>
      </div>
    </div>
  );
}

function StatCard({
  icon,
  value,
  label,
  bg,
  iconColor,
  small,
}: {
  icon: React.ReactNode;
  value: string;
  label: string;
  bg: string;
  iconColor: string;
  small?: boolean;
}) {
  return (
    <div className={`rounded-2xl ${bg} p-4`}>
      <div className={`mb-3 ${iconColor}`}>{icon}</div>
      <p
        className={`font-semibold text-zinc-100 ${small ? "truncate text-sm" : "text-2xl"}`}
        title={value}
      >
        {value}
      </p>
      <p className="mt-0.5 text-xs text-zinc-400">{label}</p>
    </div>
  );
}
