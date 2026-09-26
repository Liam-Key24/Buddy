import { cn } from "../../lib/cn";

export function SegmentedChoice<T extends string>({
  value,
  onChange,
  options,
  disabled,
  ariaLabel,
  className,
}: {
  value: T;
  onChange: (next: T) => void;
  options: { value: T; label: string }[];
  disabled?: boolean;
  ariaLabel: string;
  className?: string;
}) {
  return (
    <div
      role="group"
      aria-label={ariaLabel}
      className={cn("settings-segment", disabled && "settings-dim", className)}
      style={{ gridTemplateColumns: `repeat(${options.length}, minmax(0, 1fr))` }}
    >
      {options.map((opt) => {
        const active = value === opt.value;
        return (
          <button
            key={opt.value}
            type="button"
            disabled={disabled}
            aria-pressed={active}
            onClick={() => onChange(opt.value)}
            className={cn("settings-segment-btn", active && "settings-segment-btn-on")}
          >
            {opt.label}
          </button>
        );
      })}
    </div>
  );
}
