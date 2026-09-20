import type { ButtonHTMLAttributes, ReactNode } from "react";
import { cn } from "../../lib/cn";

type IconButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  label: string;
  children: ReactNode;
  size?: "sm" | "md";
};

export function IconButton({
  label,
  children,
  size = "md",
  className,
  type = "button",
  ...rest
}: IconButtonProps) {
  return (
    <button
      type={type}
      aria-label={label}
      title={label}
      className={cn(
        "inline-flex shrink-0 items-center justify-center rounded-lg text-muted hover:bg-raised-soft hover:text-ink disabled:opacity-40",
        size === "sm" ? "size-7" : "size-8",
        className,
      )}
      {...rest}
    >
      {children}
    </button>
  );
}
