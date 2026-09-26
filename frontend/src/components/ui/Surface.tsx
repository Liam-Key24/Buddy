import type { HTMLAttributes, ReactNode } from "react";
import { cn } from "../../lib/cn";

type SurfaceProps = HTMLAttributes<HTMLDivElement> & {
  children: ReactNode;
  pad?: boolean;
  /** Default card; panel = softer calendar-sidebar tile */
  tone?: "default" | "panel";
};

export function Surface({
  children,
  className,
  pad = true,
  tone = "default",
  ...rest
}: SurfaceProps) {
  return (
    <div
      className={cn(
        tone === "panel"
          ? "rounded-3xl border-0 bg-raised/90 shadow-[0_8px_24px_rgb(10_16_14/0.18)]"
          : "rounded-card bg-raised/80 shadow-[0_8px_24px_rgb(10_16_14/0.18)]",
        pad && "p-4",
        className,
      )}
      {...rest}
    >
      {children}
    </div>
  );
}
