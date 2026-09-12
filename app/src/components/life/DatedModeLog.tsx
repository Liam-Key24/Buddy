import { Fragment, useState, type ReactNode } from "react";
import { ArrowLeft, CaretRight, Plus } from "@phosphor-icons/react";
import { formatDay, todayIso } from "../../lib/dates";

export type DatedItem = { date: string; at: number };

export function groupByDate<T extends DatedItem>(items: T[]) {
  const map = new Map<string, T[]>();
  for (const item of items) {
    const list = map.get(item.date) ?? [];
    list.push(item);
    map.set(item.date, list);
  }
  return [...map.entries()]
    .sort((a, b) => b[0].localeCompare(a[0]))
    .map(([date, group]) => ({
      date,
      items: group.sort((a, b) => b.at - a.at),
    }));
}

export function DatedModeLog<T extends DatedItem>({
  modes,
  mode,
  onModeChange,
  onAdd,
  items,
  itemMatchesMode,
  renderItem,
  itemKey,
}: {
  modes: readonly { id: string; label: string }[];
  mode: string;
  onModeChange: (id: string) => void;
  onAdd: (date: string) => void;
  items: T[];
  itemMatchesMode: (item: T, mode: string) => boolean;
  renderItem: (item: T) => ReactNode;
  itemKey: (item: T) => string;
}) {
  const [openDate, setOpenDate] = useState<string | null>(null);
  const filtered = items.filter((item) => itemMatchesMode(item, mode));
  const groups = groupByDate(filtered);
  const dayItems = openDate
    ? items.filter((item) => item.date === openDate).sort((a, b) => b.at - a.at)
    : [];

  return (
    <div className="space-y-3">
      <div className="flex items-center gap-2">
        <div className="grid min-w-0 flex-1 grid-cols-2 gap-2">
          {modes.map((m) => (
            <button
              key={m.id}
              type="button"
              onClick={() => onModeChange(m.id)}
              className={`rounded-xl py-2.5 text-sm ${
                mode === m.id ? "bg-blue-500 text-white" : "bg-zinc-800 text-zinc-400"
              }`}
            >
              {m.label}
            </button>
          ))}
        </div>
        <button
          type="button"
          title={`Log ${mode}`}
          aria-label={`Log ${mode}`}
          onClick={() => onAdd(openDate ?? todayIso())}
          className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-blue-500 text-white"
        >
          <Plus size={16} weight="bold" />
        </button>
      </div>

      {openDate ? (
        <div className="space-y-3">
          <button
            type="button"
            onClick={() => setOpenDate(null)}
            className="flex items-center gap-1.5 text-sm text-zinc-300"
          >
            <ArrowLeft size={14} />
            {formatDay(openDate)}
          </button>
          {dayItems.length === 0 && (
            <p className="text-xs text-zinc-600">Nothing logged</p>
          )}
          {dayItems.map((item) => (
            <Fragment key={itemKey(item)}>{renderItem(item)}</Fragment>
          ))}
        </div>
      ) : (
        <div className="space-y-4">
          {groups.length === 0 && (
            <p className="text-xs text-zinc-600">Nothing logged</p>
          )}
          {groups.map((group) => (
            <section key={group.date} className="space-y-2">
              <button
                type="button"
                onClick={() => setOpenDate(group.date)}
                className="flex w-full items-center justify-between text-xs text-zinc-500 hover:text-zinc-200"
              >
                <span>{formatDay(group.date)}</span>
                <CaretRight size={12} />
              </button>
              {group.items.map((item) => (
                <Fragment key={itemKey(item)}>{renderItem(item)}</Fragment>
              ))}
            </section>
          ))}
        </div>
      )}
    </div>
  );
}
