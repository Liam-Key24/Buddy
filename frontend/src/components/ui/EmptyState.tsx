import type { ReactNode } from "react";
import { cn } from "../../lib/cn";

type EmptyStateProps = {
  icon?: ReactNode;
  children: ReactNode;
  action?: ReactNode;
  className?: string;
};

export function EmptyState({ icon, children, action, className }: EmptyStateProps) {
  return (
    <div className={cn("flex flex-col items-start gap-2 py-3 text-sm text-muted", className)}>
      {icon && <span className="text-mint-dim">{icon}</span>}
      <p className="m-0">{children}</p>
      {action}
    </div>
  );
}
