import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from "react";
import { cn } from "../../lib/cn";

type Tone = "primary" | "ghost" | "danger" | "raised";

const tones: Record<Tone, string> = {
  primary:
    "bg-mint text-page-deep hover:bg-mint-dim disabled:opacity-40 disabled:hover:bg-mint",
  ghost: "bg-transparent text-ink-soft hover:bg-raised-soft disabled:opacity-40",
  danger: "bg-danger/15 text-danger hover:bg-danger/25 disabled:opacity-40",
  raised: "bg-raised text-ink hover:bg-raised-soft disabled:opacity-40",
};

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  tone?: Tone;
  block?: boolean;
  children: ReactNode;
};

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { tone = "raised", block, className, type = "button", children, ...rest },
  ref,
) {
  return (
    <button
      ref={ref}
      type={type}
      className={cn(
        "inline-flex items-center justify-center gap-1.5 rounded-pill px-3 py-1.5 text-sm font-medium transition-colors",
        block && "w-full",
        tones[tone],
        className,
      )}
      {...rest}
    >
      {children}
    </button>
  );
});
