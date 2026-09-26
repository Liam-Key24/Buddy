import type { ButtonHTMLAttributes, ReactNode } from "react";
import { cn } from "../../lib/cn";

type IconButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  label: string;
  children: ReactNode;
  size?: "sm" | "md";
  /** soft = raised chip; ghost = no fill until hover */
  tone?: "default" | "soft" | "ghost";
};

export function IconButton({
  label,
  children,
  size = "md",
  tone = "default",
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
        "inline-flex shrink-0 items-center justify-center rounded-lg disabled:opacity-40",
        size === "sm" ? "size-7" : "size-8",
        tone === "soft" && "bg-raised-soft text-ink",
        tone === "ghost" && "text-muted hover:bg-raised-soft hover:text-ink",
        tone === "default" && "text-muted hover:bg-raised-soft hover:text-ink",
        className,
      )}
      {...rest}
    >
      {children}
    </button>
  );
}
