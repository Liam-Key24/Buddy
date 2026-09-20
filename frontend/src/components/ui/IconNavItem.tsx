import type { ReactNode } from "react";
import { NavLink } from "react-router-dom";
import { cn } from "../../lib/cn";

type IconNavItemProps = {
  to?: string;
  end?: boolean;
  icon: ReactNode;
  label: string;
  active?: boolean;
  onClick?: () => void;
  collapsed?: boolean;
};

const row =
  "flex w-full items-center gap-2.5 rounded-xl px-2.5 py-1.5 text-sm text-ink-soft hover:bg-sidebar-hover hover:text-ink";

export function IconNavItem({
  to,
  end,
  icon,
  label,
  active,
  onClick,
  collapsed,
}: IconNavItemProps) {
  const inner = (
    <>
      <span className="grid size-5 shrink-0 place-items-center text-mint-dim">{icon}</span>
      {!collapsed && <span className="truncate">{label}</span>}
    </>
  );

  if (to) {
    return (
      <NavLink
        to={to}
        end={end}
        aria-label={label}
        title={label}
        onClick={onClick}
        className={({ isActive }) =>
          cn(row, (isActive || active) && "bg-raised-soft text-mint")
        }
      >
        {inner}
      </NavLink>
    );
  }

  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={onClick}
      className={cn(row, active && "bg-raised-soft text-mint")}
    >
      {inner}
    </button>
  );
}
