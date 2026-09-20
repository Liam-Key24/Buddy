import { Eye, PencilSimple, Plus, Trash } from "@phosphor-icons/react";
import { useState } from "react";
import type { Category } from "../api";
import { CategoryIcon } from "./CategoryIcon";
import { Button } from "./ui/Button";
import { IconButton } from "./ui/IconButton";
import { SectionHead } from "./ui/SectionHead";
import {
  CATEGORY_COLORS,
  iconFromName,
  isSystemCategory,
  UNCATEGORIZED_KEY,
} from "../lib/categories";
import { cn } from "../lib/cn";

export type CategoryDraft = {
  id?: string;
  name: string;
  color: string;
  keywords: string;
  icon: string;
};

type Row = {
  id: string;
  name: string;
  color: string;
  icon?: string;
  count: number;
  system?: boolean;
};

type CategoryPanelProps = {
  categories: Category[];
  counts: Record<string, number>;
  uncategorizedCount: number;
  enabled: Record<string, boolean>;
  total: number;
  busy?: boolean;
  onToggle: (id: string, on: boolean) => void;
  onShowAll: () => void;
  onSave: (draft: CategoryDraft) => Promise<void>;
  onRequestDelete: (cat: Category) => void;
};

function ColorDots({
  value,
  onChange,
}: {
  value: string;
  onChange: (c: string) => void;
}) {
  return (
    <div className="flex flex-wrap gap-1.5">
      {CATEGORY_COLORS.map((c) => (
        <button
          key={c}
          type="button"
          className={cn(
            "size-5 rounded-full",
            value === c && "ring-2 ring-mint ring-offset-1 ring-offset-raised",
          )}
          style={{ background: c }}
          onClick={() => onChange(c)}
          aria-label={c}
        />
      ))}
    </div>
  );
}

function CategoryForm({
  initial,
  busy,
  onCancel,
  onSubmit,
}: {
  initial?: CategoryDraft;
  busy?: boolean;
  onCancel: () => void;
  onSubmit: (draft: CategoryDraft) => Promise<void>;
}) {
  const [name, setName] = useState(initial?.name ?? "");
  const [keywords, setKeywords] = useState(initial?.keywords ?? "");
  const [color, setColor] = useState(initial?.color ?? CATEGORY_COLORS[0]);

  return (
    <div className="mt-3 flex flex-col gap-2 border-t border-hairline pt-3">
      <input
        className="field"
        value={name}
        onChange={(e) => setName(e.target.value)}
        placeholder="Name"
        autoFocus
      />
      <input
        className="field"
        value={keywords}
        onChange={(e) => setKeywords(e.target.value)}
        placeholder="Keywords (comma-separated)"
      />
      <p className="m-0 text-[11px] text-muted">
        Used to auto-tag events from their titles.
      </p>
      <ColorDots value={color} onChange={setColor} />
      <div className="flex gap-2">
        <Button tone="ghost" onClick={onCancel}>
          Cancel
        </Button>
        <Button
          tone="primary"
          disabled={busy || !name.trim()}
          onClick={() => {
            void onSubmit({
              id: initial?.id,
              name: name.trim(),
              color,
              keywords: keywords.trim(),
              icon: initial?.icon || iconFromName(name),
            }).catch(() => {
              /* stay open; parent sets error */
            });
          }}
        >
          {initial?.id ? "Save" : "Add"}
        </Button>
      </div>
    </div>
  );
}

function FilterRow({
  row,
  on,
  pct,
  onToggle,
  onEdit,
  onDelete,
}: {
  row: Row;
  on: boolean;
  pct: number;
  onToggle: (on: boolean) => void;
  onEdit?: () => void;
  onDelete?: () => void;
}) {
  return (
    <li className="flex items-center gap-1.5">
      <button
        type="button"
        role="switch"
        aria-checked={on}
        aria-label={`${on ? "Hide" : "Show"} ${row.name}`}
        onClick={() => onToggle(!on)}
        className={cn(
          "flex min-w-0 flex-1 items-center gap-2 rounded-xl px-1 py-1 text-left hover:bg-raised-soft/60",
          !on && "opacity-45",
        )}
      >
        <span
          className={cn(
            "grid size-4 shrink-0 place-items-center rounded-full border border-hairline",
            on && "bg-mint/20",
          )}
          style={on ? { borderColor: row.color } : undefined}
        >
          {on && (
            <span className="size-2 rounded-full" style={{ background: row.color }} />
          )}
        </span>
        <span
          className="grid size-6 shrink-0 place-items-center rounded-full"
          style={{ background: `color-mix(in srgb, ${row.color} 28%, transparent)` }}
        >
          <CategoryIcon name={row.icon || row.name} size={12} />
        </span>
        <span className="w-[4.5rem] shrink-0 truncate text-sm">{row.name}</span>
        <span className="h-1.5 min-w-0 flex-1 overflow-hidden rounded-full bg-raised-soft">
          <span
            className="block h-full rounded-full"
            style={{
              width: `${on ? Math.max(pct, row.count ? 10 : 0) : 0}%`,
              background: row.color,
            }}
          />
        </span>
        <span className="w-5 shrink-0 text-right text-[10px] tabular-nums text-muted">
          {row.count}
        </span>
      </button>
      {onEdit && (
        <IconButton label={`Edit ${row.name}`} size="sm" onClick={onEdit}>
          <PencilSimple size={14} />
        </IconButton>
      )}
      {onDelete && (
        <IconButton label={`Delete ${row.name}`} size="sm" onClick={onDelete}>
          <Trash size={14} />
        </IconButton>
      )}
    </li>
  );
}

export function CategoryPanel({
  categories,
  counts,
  uncategorizedCount,
  enabled,
  total,
  busy,
  onToggle,
  onShowAll,
  onSave,
  onRequestDelete,
}: CategoryPanelProps) {
  const [form, setForm] = useState<"add" | Category | null>(null);
  const denom = Math.max(1, total);

  const rows: Row[] = [
    ...categories.map((c) => ({
      id: c.id,
      name: c.name,
      color: c.color,
      icon: c.icon,
      count: counts[c.id] || 0,
      system: isSystemCategory(c.name),
    })),
    {
      id: UNCATEGORIZED_KEY,
      name: "Uncategorized",
      color: "#7a8e87",
      icon: "circle",
      count: uncategorizedCount,
      system: true,
    },
  ];

  return (
    <>
      <SectionHead
        title="Categories"
        action={
          <div className="flex items-center gap-0.5">
            <IconButton label="Show all categories" size="sm" onClick={onShowAll}>
              <Eye size={14} />
            </IconButton>
            <IconButton label="Add category" size="sm" onClick={() => setForm("add")}>
              <Plus size={14} weight="bold" />
            </IconButton>
          </div>
        }
      />
      <p className="mt-0 mb-3 text-[11px] text-muted">
        Tap to show or hide on the calendar. Busy hours block proposing separately.
      </p>
      <ul className="m-0 flex list-none flex-col gap-1.5 p-0">
        {rows.map((row) => (
          <FilterRow
            key={row.id}
            row={row}
            on={enabled[row.id] !== false}
            pct={Math.min(100, Math.round((row.count / denom) * 100))}
            onToggle={(on) => onToggle(row.id, on)}
            onEdit={
              row.id !== UNCATEGORIZED_KEY
                ? () => setForm(categories.find((c) => c.id === row.id) || null)
                : undefined
            }
            onDelete={
              row.id !== UNCATEGORIZED_KEY && !row.system
                ? () => {
                    const cat = categories.find((c) => c.id === row.id);
                    if (cat) onRequestDelete(cat);
                  }
                : undefined
            }
          />
        ))}
      </ul>
      {form === "add" && (
        <CategoryForm
          busy={busy}
          onCancel={() => setForm(null)}
          onSubmit={async (draft) => {
            await onSave(draft);
            setForm(null);
          }}
        />
      )}
      {form && form !== "add" && (
        <CategoryForm
          initial={{
            id: form.id,
            name: form.name,
            color: form.color,
            keywords: form.keywords,
            icon: form.icon,
          }}
          busy={busy}
          onCancel={() => setForm(null)}
          onSubmit={async (draft) => {
            await onSave(draft);
            setForm(null);
          }}
        />
      )}
    </>
  );
}
