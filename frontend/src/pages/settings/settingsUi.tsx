import type { ButtonHTMLAttributes, ReactNode } from "react";
import { ErrorBanner } from "../../components/ui/ErrorBanner";
import { IconButton } from "../../components/ui/IconButton";
import { cn } from "../../lib/cn";
import { useSettingsChrome } from "./SettingsLayout";

export { SegmentedChoice } from "../../components/ui/SegmentedChoice";

export function SettingsMasonry({ children }: { children: ReactNode }) {
  return <div className="settings-masonry">{children}</div>;
}

export function SettingsPageHead({
  title,
  subtitle,
  action,
}: {
  title: string;
  subtitle?: string;
  action?: ReactNode;
}) {
  const chrome = useSettingsChrome();

  return (
    <div className="settings-page-head">
      <div className="settings-page-head-row">
        <div className="settings-page-head-side">{chrome?.mobileMenu}</div>
        <h1 className="settings-title">{title}</h1>
        <div className="settings-page-head-side settings-page-head-side-end">{action}</div>
      </div>
      {subtitle ? <p className="settings-subtitle">{subtitle}</p> : null}
    </div>
  );
}

export function SettingsLabel({ children }: { children: ReactNode }) {
  return <label className="settings-label">{children}</label>;
}

export function SettingsHint({ children }: { children: ReactNode }) {
  return <p className="settings-hint">{children}</p>;
}

export function SettingsMeta({ children, className }: { children: ReactNode; className?: string }) {
  return <span className={cn("settings-meta", className)}>{children}</span>;
}

export function SettingsError({ children }: { children: ReactNode }) {
  return <ErrorBanner>{children}</ErrorBanner>;
}

export function SettingsCta({
  children,
  className,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button type="button" className={cn("settings-cta", className)} {...props}>
      {children}
    </button>
  );
}

export function SettingsSoonBtn({ children }: { children: ReactNode }) {
  return (
    <button type="button" disabled className="settings-cta-soon">
      {children}
    </button>
  );
}

export function SettingsChip({
  on,
  children,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { on?: boolean }) {
  return (
    <button type="button" className={on ? "settings-chip-on" : "settings-chip"} {...props}>
      {children}
    </button>
  );
}

export function SettingsAvatar({ children }: { children: ReactNode }) {
  return (
    <div className="settings-avatar" aria-hidden>
      {children}
    </div>
  );
}

export function SettingsProfileRow({
  avatar,
  title,
  subtitle,
}: {
  avatar: ReactNode;
  title: string;
  subtitle: string;
}) {
  return (
    <div className="settings-profile">
      <SettingsAvatar>{avatar}</SettingsAvatar>
      <div className="min-w-0 text-sm">
        <strong className="block">{title}</strong>
        <SettingsMeta>{subtitle}</SettingsMeta>
      </div>
    </div>
  );
}

export function SettingsDl({ rows }: { rows: { label: string; value: ReactNode }[] }) {
  return (
    <dl className="settings-dl">
      {rows.map((row) => (
        <div key={row.label}>
          <dt className="settings-dt">{row.label}</dt>
          <dd className="settings-dd">{row.value}</dd>
        </div>
      ))}
    </dl>
  );
}

/** Local-only placeholder switch — not persisted yet. */
export function PlaceholderToggle({
  checked,
  onChange,
  label,
  hint,
  disabled,
}: {
  checked: boolean;
  onChange: (next: boolean) => void;
  label: string;
  hint?: string;
  disabled?: boolean;
}) {
  return (
    <label className={cn("settings-toggle", disabled && "cursor-not-allowed settings-dim")}>
      <span className="min-w-0">
        <span className="block text-sm text-ink">{label}</span>
        {hint ? <span className="mt-0.5 block settings-meta">{hint}</span> : null}
      </span>
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        aria-label={label}
        disabled={disabled}
        onClick={() => onChange(!checked)}
        className={cn("settings-toggle-track", checked && "settings-toggle-track-on")}
      >
        <span className={cn("settings-toggle-knob", checked && "settings-toggle-knob-on")} />
      </button>
    </label>
  );
}

export function SettingsWeekDay({
  dow,
  day,
  selected,
  today,
  dots,
  disabled,
  onClick,
}: {
  dow: string;
  day: number;
  selected?: boolean;
  today?: boolean;
  dots: number;
  disabled?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onClick}
      className={cn(
        "settings-week-day",
        selected && "settings-week-day-on",
        today && !selected && "settings-week-day-today",
      )}
    >
      <span className="settings-week-dow">{dow}</span>
      <span className="settings-week-num">{day}</span>
      <span className="flex h-1.5 gap-0.5">
        {dots === 0 ? (
          <span className="size-1 rounded-full bg-transparent" />
        ) : (
          Array.from({ length: Math.min(dots, 3) }).map((_, i) => (
            <span key={i} className="size-1 rounded-full bg-mint" />
          ))
        )}
      </span>
    </button>
  );
}

export function SettingsShiftRow({
  place,
  range,
  onRemove,
  disabled,
  removeIcon,
}: {
  place: string;
  range: string;
  onRemove: () => void;
  disabled?: boolean;
  removeIcon: ReactNode;
}) {
  return (
    <li className="settings-shift">
      <div className="min-w-0">
        <strong className="block truncate text-sm text-ink">{place}</strong>
        <SettingsMeta className="tabular-nums">{range}</SettingsMeta>
      </div>
      <IconButton
        label={`Remove ${place}`}
        size="sm"
        tone="ghost"
        disabled={disabled}
        onClick={onRemove}
      >
        {removeIcon}
      </IconButton>
    </li>
  );
}
