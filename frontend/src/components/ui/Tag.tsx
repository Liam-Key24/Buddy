import { X } from "@phosphor-icons/react";
import type { ReactNode } from "react";
import { cn } from "../../lib/cn";

type Tone = "mint" | "raised" | "ok" | "warn" | "danger";

const tones: Record<Tone, string> = {
  mint: "bg-mint/15 text-mint",
  raised: "bg-raised-soft text-ink-soft",
  ok: "bg-ok/15 text-ok",
  warn: "bg-warn/15 text-warn",
  danger: "bg-danger/15 text-danger",
};

type TagProps = {
  children: ReactNode;
  icon?: ReactNode;
  tone?: Tone;
  color?: string;
  onDismiss?: () => void;
  className?: string;
};

export function Tag({ children, icon, tone = "raised", color, onDismiss, className }: TagProps) {
  const custom = color
    ? {
        background: `color-mix(in srgb, ${color} 28%, transparent)`,
        color,
        borderColor: `color-mix(in srgb, ${color} 40%, transparent)`,
      }
    : undefined;

  return (
    <span
      className={cn(
        "inline-flex max-w-full items-center gap-1 rounded-pill border border-transparent px-2 py-0.5 text-xs font-medium",
        !color && tones[tone],
        className,
      )}
      style={custom}
    >
      {icon}
      <span className="truncate">{children}</span>
      {onDismiss && (
        <button
          type="button"
          aria-label="Remove"
          onClick={onDismiss}
          className="grid size-3.5 place-items-center rounded-full opacity-70 hover:opacity-100"
        >
          <X size={10} weight="bold" />
        </button>
      )}
    </span>
  );
}
