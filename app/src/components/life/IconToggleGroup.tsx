import type { ComponentType } from "react";

export type IconComp = ComponentType<{
  size?: number;
  weight?: "fill" | "regular";
  className?: string;
}>;

export type IconToggleItem = {
  id: string;
  label: string;
  Icon: IconComp;
  active?: string;
};

export function IconToggleGroup({
  value,
  onChange,
  items,
  size = 14,
}: {
  value: string | null;
  onChange: (id: string) => void;
  items: readonly IconToggleItem[];
  size?: number;
}) {
  return (
    <div className="flex shrink-0 items-center gap-0.5">
      {items.map(({ id, label, Icon, active = "text-blue-400" }) => {
        const selected = value === id;
        return (
          <button
            key={id}
            type="button"
            title={label}
            aria-label={label}
            aria-pressed={selected}
            onClick={() => onChange(id)}
            className={`rounded-md p-0.5 ${
              selected ? active : "text-zinc-600 hover:text-zinc-300"
            }`}
          >
            <Icon size={size} weight={selected ? "fill" : "regular"} />
          </button>
        );
      })}
    </div>
  );
}

export function IconFilterRow({
  allTitle,
  AllIcon,
  value,
  onChange,
  items,
}: {
  allTitle: string;
  AllIcon: IconComp;
  value: string | null;
  onChange: (id: string | null) => void;
  items: readonly IconToggleItem[];
}) {
  return (
    <div className="flex items-center gap-0.5" role="group">
      <button
        type="button"
        title={allTitle}
        aria-label={allTitle}
        aria-pressed={value === null}
        onClick={() => onChange(null)}
        className={`rounded-md p-1 ${value === null ? "text-blue-400" : "text-zinc-600 hover:text-zinc-300"}`}
      >
        <AllIcon size={15} weight={value === null ? "fill" : "regular"} />
      </button>
      {items.map(({ id, label, Icon, active = "text-blue-400" }) => {
        const selected = value === id;
        return (
          <button
            key={id}
            type="button"
            title={label}
            aria-label={label}
            aria-pressed={selected}
            onClick={() => onChange(selected ? null : id)}
            className={`rounded-md p-1 ${selected ? active : "text-zinc-600 hover:text-zinc-300"}`}
          >
            <Icon size={15} weight={selected ? "fill" : "regular"} />
          </button>
        );
      })}
    </div>
  );
}
