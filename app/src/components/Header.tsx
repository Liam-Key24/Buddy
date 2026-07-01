import { useAppStore } from "../stores/useAppStore";

function StatusDot({ label, status }: { label: string; status: string }) {
  const color =
    status === "online"
      ? "bg-emerald-500"
      : status === "checking"
        ? "bg-amber-500 animate-pulse"
        : "bg-red-500";

  return (
    <div className="flex items-center gap-1.5 text-xs text-gray-400">
      <span className={`h-2 w-2 rounded-full ${color}`} />
      {label}
    </div>
  );
}

export function Header() {
  const { mlxStatus, brainStatus } = useAppStore();

  return (
    <header className="flex h-12 shrink-0 items-center justify-between border-b border-gray-800 px-4">
      <h1 className="text-sm font-semibold text-gray-200">Buddy</h1>
      <div className="flex items-center gap-4">
        <StatusDot label="MLX" status={mlxStatus} />
        <StatusDot label="Brain" status={brainStatus} />
      </div>
    </header>
  );
}
