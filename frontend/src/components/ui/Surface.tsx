import type { HTMLAttributes, ReactNode } from "react";
import { cn } from "../../lib/cn";

type SurfaceProps = HTMLAttributes<HTMLDivElement> & {
  children: ReactNode;
  pad?: boolean;
};

export function Surface({ children, className, pad = true, ...rest }: SurfaceProps) {
  return (
    <div
      className={cn(
        "rounded-card bg-raised/80 shadow-[0_8px_24px_rgb(10_16_14/0.18)]",
        pad && "p-4",
        className,
      )}
      {...rest}
    >
      {children}
    </div>
  );
}
