import type { HTMLAttributes, ReactNode } from "react";
import { cn } from "../../lib/cn";

type FrostFloatProps = HTMLAttributes<HTMLDivElement> & {
  children: ReactNode;
};

export function FrostFloat({ children, className, ...rest }: FrostFloatProps) {
  return (
    <div className={cn("frost-float rounded-float", className)} {...rest}>
      {children}
    </div>
  );
}
