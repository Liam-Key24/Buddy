import type { ReactNode } from "react";
import { cn } from "../../lib/cn";

export function ErrorBanner({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  if (!children) return null;
  return <div className={cn("settings-error", className)}>{children}</div>;
}
