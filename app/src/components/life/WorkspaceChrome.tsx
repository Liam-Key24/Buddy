import type { ComponentType, ReactNode } from "react";

type IconComp = ComponentType<{ size?: number; className?: string }>;

export function WorkspaceNavItem({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`truncate rounded-lg px-2 py-1.5 text-left text-sm ${
        active ? "bg-blue-500/15 text-blue-400" : "text-zinc-400 hover:bg-zinc-800"
      }`}
    >
      {children}
    </button>
  );
}

export function WorkspacePaneHeader({
  Icon,
  iconClassName = "text-blue-400",
  title,
  children,
}: {
  Icon: IconComp;
  iconClassName?: string;
  title?: ReactNode;
  children?: ReactNode;
}) {
  return (
    <div className="mb-3 flex items-center gap-2">
      <Icon size={18} className={iconClassName} />
      {children ?? <h3 className="text-sm font-medium text-zinc-200">{title}</h3>}
    </div>
  );
}
