import { useState } from "react";
import {
  ArrowDown,
  CheckSquare,
  Clock,
  Fire,
  Minus,
  Plus,
  Trash,
  Warning,
} from "@phosphor-icons/react";
import { LifeChatSplit } from "../components/LifeChatSplit";
import { IconFilterRow, IconToggleGroup } from "../components/life/IconToggleGroup";
import { TASK_STATUSES } from "../components/life/statusOptions";
import { useLifePage } from "../hooks/useLifePage";
import { shortDate, todayIso } from "../lib/dates";
import { isOverdue, useTodoStore } from "../stores/useTodoStore";
import type { Todo } from "../lib/lifeApi";

const PRIORITIES = [
  { id: "low", label: "Low", Icon: ArrowDown, active: "text-zinc-400" },
  { id: "medium", label: "Medium", Icon: Minus, active: "text-blue-400" },
  { id: "high", label: "High", Icon: Warning, active: "text-amber-400" },
  { id: "critical", label: "Critical", Icon: Fire, active: "text-rose-400" },
] as const;

export function TodoPage() {
  const { todos, loading, refresh, save, complete, remove } = useTodoStore();
  const [status, setStatus] = useState<string | null>(null);
  const [priority, setPriority] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  const [title, setTitle] = useState("");
  const [draftPriority, setDraftPriority] = useState("medium");

  useLifePage({
    load: refresh,
    loadDeps: [refresh],
    context: "Page: todo. Add/update tasks with todo.add and todo.update. List with todo.list.",
  });

  const today = todayIso();
  const filtered = todos.filter((t) => {
    if (status && t.status !== status) return false;
    if (priority && t.priority !== priority) return false;
    return true;
  });

  function dismissAdd() {
    setTitle("");
    setDraftPriority("medium");
    setAdding(false);
  }

  async function handleAdd() {
    if (!title.trim()) {
      dismissAdd();
      return;
    }
    await save({
      title: title.trim(),
      description: null,
      deadline: null,
      priority: draftPriority,
      category: "general",
      recurrence: "none",
      status: "not_started",
    });
    dismissAdd();
  }

  return (
    <LifeChatSplit>
    <div className="min-h-0 flex-1 overflow-y-auto p-4">
      <div className="mx-auto max-w-3xl space-y-4">
        <div
          onBlur={(e) => {
            if (!adding) return;
            if (e.currentTarget.contains(e.relatedTarget as Node | null)) return;
            if (!title.trim()) dismissAdd();
          }}
        >
        <div className="flex items-center gap-2">
          <CheckSquare size={18} weight="fill" className="text-blue-400" />
          <h3 className="min-w-0 flex-1 text-sm font-medium text-zinc-200">To-Do</h3>
          <button
            type="button"
            title="Add task"
            aria-label="Add task"
            onClick={() => (adding ? dismissAdd() : setAdding(true))}
            className="rounded-md p-1 text-blue-400 hover:bg-zinc-800"
          >
            <Plus size={16} weight="bold" />
          </button>
        </div>

        <div className="mt-4 flex items-center gap-3">
          <IconFilterRow
            allTitle="All"
            AllIcon={CheckSquare}
            value={status}
            onChange={setStatus}
            items={TASK_STATUSES}
          />
          <IconFilterRow
            allTitle="All priority"
            AllIcon={Minus}
            value={priority}
            onChange={setPriority}
            items={PRIORITIES}
          />
        </div>

        {adding && (
          <div className="mt-4 rounded-xl border border-zinc-800 bg-zinc-950/40 px-3 py-2.5">
            <input
              autoFocus
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  void handleAdd();
                }
                if (e.key === "Escape") {
                  e.preventDefault();
                  dismissAdd();
                }
              }}
              placeholder="Task"
              className="w-full bg-transparent text-sm text-zinc-100 outline-none"
            />
            <div className="mt-2 flex items-center justify-between gap-2">
              <div className="flex items-center gap-0.5">
                {PRIORITIES.map(({ id, label, Icon, active }) => {
                  const selected = draftPriority === id;
                  return (
                    <button
                      key={id}
                      type="button"
                      title={label}
                      aria-label={label}
                      aria-pressed={selected}
                      onClick={() => setDraftPriority(id)}
                      className={`rounded-md p-1 ${selected ? active : "text-zinc-600 hover:text-zinc-300"}`}
                    >
                      <Icon size={15} weight={selected ? "fill" : "regular"} />
                    </button>
                  );
                })}
              </div>
              <button
                type="button"
                onClick={() => void handleAdd()}
                disabled={!title.trim()}
                className="rounded-lg bg-blue-500 px-2.5 py-1 text-xs text-white disabled:opacity-40"
              >
                Save
              </button>
            </div>
          </div>
        )}
        </div>

        {loading ? (
          <p className="px-1 text-xs text-zinc-600">Loading…</p>
        ) : filtered.length === 0 ? (
          <p className="px-1 text-xs text-zinc-600">No tasks yet</p>
        ) : (
          <div className="space-y-2">
            {filtered.map((todo) => (
              <TodoCard
                key={todo.id}
                todo={todo}
                overdue={isOverdue(todo, today)}
                onStatus={(s) => {
                  if (s === "completed") void complete(todo.id);
                  else void save({ ...todo, title: todo.title, status: s });
                }}
                onPriority={(p) => void save({ ...todo, title: todo.title, priority: p })}
                onDelete={() => remove(todo.id)}
              />
            ))}
          </div>
        )}
      </div>
    </div>
    </LifeChatSplit>
  );
}

function TodoCard({
  todo,
  overdue,
  onStatus,
  onPriority,
  onDelete,
}: {
  todo: Todo;
  overdue: boolean;
  onStatus: (s: string) => void;
  onPriority: (p: string) => void;
  onDelete: () => void;
}) {
  const due = shortDate(todo.deadline);
  return (
    <div
      className={`flex items-center gap-2.5 rounded-xl border bg-zinc-950/40 px-3 py-2.5 ${
        overdue ? "border-rose-500/40" : "border-zinc-800"
      }`}
    >
      <IconToggleGroup value={todo.status} onChange={onStatus} items={TASK_STATUSES} />
      <div className="min-w-0 flex-1">
        <p className={`truncate text-sm ${todo.status === "completed" ? "text-zinc-500 line-through" : "text-zinc-100"}`}>
          {todo.title}
        </p>
        <div className="mt-0.5 flex items-center gap-2 text-[11px] text-zinc-500">
          {due && (
            <span className={`flex items-center gap-1 ${overdue ? "text-rose-400" : ""}`}>
              <Clock size={12} />
              {due}
            </span>
          )}
          {overdue && <span className="text-rose-400">Overdue</span>}
        </div>
      </div>
      <div className="flex shrink-0 items-center gap-0.5">
        {PRIORITIES.map(({ id, label, Icon, active }) => {
          const selected = todo.priority === id;
          return (
            <button
              key={id}
              type="button"
              title={label}
              aria-label={label}
              aria-pressed={selected}
              onClick={() => onPriority(id)}
              className={`rounded-md p-0.5 ${selected ? active : "text-zinc-600 hover:text-zinc-300"}`}
            >
              <Icon size={14} weight={selected ? "fill" : "regular"} />
            </button>
          );
        })}
        <button
          type="button"
          title="Delete"
          aria-label="Delete"
          onClick={onDelete}
          className="rounded-md p-1 text-zinc-600 hover:bg-zinc-800 hover:text-rose-400"
        >
          <Trash size={14} />
        </button>
      </div>
    </div>
  );
}
