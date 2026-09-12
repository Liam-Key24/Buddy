import { useMemo, useState } from "react";
import {
  ArrowCounterClockwise,
  Basket,
  Briefcase,
  Bus,
  DotsThree,
  ForkKnife,
  Gift,
  House,
  Laptop,
  Lightning,
  Phone,
  PiggyBank,
  ShoppingCart,
  Trash,
  Wallet,
} from "@phosphor-icons/react";
import { LifeChatSplit } from "../components/LifeChatSplit";
import { useLifePage } from "../hooks/useLifePage";
import { gbp, shortDate } from "../lib/dates";
import {
  moneyAnalyze,
  moneyDelete,
  moneyDeletePot,
  moneyList,
  moneySummary,
  moneyUpsert,
  moneyUpsertPot,
  type MoneyAnalysis,
  type MoneyEntry,
  type MoneyPotShare,
  type MoneySummary,
} from "../lib/lifeApi";

const MONTHS = [
  "January",
  "February",
  "March",
  "April",
  "May",
  "June",
  "July",
  "August",
  "September",
  "October",
  "November",
  "December",
];

const EXPENSE_CATS = [
  { id: "grocery", label: "Grocery", Icon: Basket },
  { id: "shop", label: "Shop", Icon: ShoppingCart },
  { id: "phone", label: "Phone bill", Icon: Phone },
  { id: "rent", label: "Rent", Icon: House },
  { id: "transport", label: "Transport", Icon: Bus },
  { id: "food", label: "Eating out", Icon: ForkKnife },
  { id: "utilities", label: "Utilities", Icon: Lightning },
  { id: "other", label: "Other", Icon: DotsThree },
] as const;

const INCOME_CATS = [
  { id: "salary", label: "Salary", Icon: Briefcase },
  { id: "freelance", label: "Freelance", Icon: Laptop },
  { id: "gift", label: "Gift", Icon: Gift },
  { id: "refund", label: "Refund", Icon: ArrowCounterClockwise },
  { id: "other", label: "Other", Icon: DotsThree },
] as const;

type Cat = (typeof EXPENSE_CATS)[number] | (typeof INCOME_CATS)[number];

function catsFor(kind: "income" | "expense"): readonly Cat[] {
  return kind === "income" ? INCOME_CATS : EXPENSE_CATS;
}

function catMeta(kind: string, category: string): Cat {
  const list = kind === "income" ? INCOME_CATS : EXPENSE_CATS;
  return list.find((c) => c.id === category) ?? list[list.length - 1];
}

function daysInMonth(year: number, month: number) {
  return new Date(year, month, 0).getDate();
}

function pad(n: number) {
  return String(n).padStart(2, "0");
}

const selectClass = "rounded-lg bg-zinc-800 px-2 py-1 text-sm text-zinc-200 outline-none";

export function Money() {
  const now = new Date();
  const years = useMemo(
    () => Array.from({ length: 6 }, (_, i) => now.getFullYear() - 2 + i),
    [now.getFullYear()],
  );
  const [year, setYear] = useState(now.getFullYear());
  const [month, setMonth] = useState(now.getMonth() + 1);
  const [entries, setEntries] = useState<MoneyEntry[]>([]);
  const [summary, setSummary] = useState<MoneySummary | null>(null);
  const [analysis, setAnalysis] = useState<MoneyAnalysis | null>(null);

  async function reload() {
    const [e, s, a] = await Promise.all([
      moneyList(year, month),
      moneySummary(year, month),
      moneyAnalyze(year, month),
    ]);
    setEntries(e);
    setSummary(s);
    setAnalysis(a);
  }

  useLifePage({
    event: "money-updated",
    load: reload,
    loadDeps: [year, month],
    context: `Page: money. Viewing ${year}-${pad(month)} GBP ledger. Read rows with money.list. Log with money.log (kind income|expense, amount in pounds). Savings split uses pots — money.pots to read, money.pot to set or add (holiday pot £200, put £50 in emergency).`,
  });

  const income = entries.filter((e) => e.kind === "income");
  const expenses = entries.filter((e) => e.kind === "expense");

  return (
    <LifeChatSplit>
    <div className="@container min-h-0 min-w-[20rem] flex-1 overflow-y-auto p-4">
      <div className="mx-auto w-full max-w-5xl space-y-4">
        <div className="flex flex-wrap items-center gap-2">
          <Wallet size={18} className="shrink-0 text-blue-400" />
          <select
            value={year}
            onChange={(e) => setYear(Number(e.target.value))}
            className={selectClass}
          >
            {years.map((y) => (
              <option key={y} value={y}>
                {y}
              </option>
            ))}
          </select>
          <select
            value={month}
            onChange={(e) => setMonth(Number(e.target.value))}
            className={selectClass}
          >
            {MONTHS.map((label, i) => (
              <option key={label} value={i + 1}>
                {label}
              </option>
            ))}
          </select>
        </div>
        {summary && (
          <div className="grid grid-cols-2 gap-3 @[36rem]:grid-cols-4">
            <Stat label="Income" value={gbp(summary.income_cents)} />
            <Stat label="Expenses" value={gbp(summary.expense_cents)} />
            <Stat label="Net" value={gbp(summary.net_cents)} />
            <Stat label="Savings" value={gbp(summary.savings_cents)} />
          </div>
        )}
        <Pots pots={summary?.pots ?? []} totalCents={summary?.pots_cents ?? 0} onChange={reload} />
        <Ledger
          title="Income"
          kind="income"
          year={year}
          month={month}
          rows={income}
          onChange={reload}
        />
        <Ledger
          title="Expenses"
          kind="expense"
          year={year}
          month={month}
          rows={expenses}
          onChange={reload}
        />
        {analysis && analysis.notes.length > 0 && (
          <div className="rounded-2xl border border-zinc-800 p-4">
            <h3 className="mb-2 text-xs uppercase tracking-wider text-zinc-500">Buddy analysis</h3>
            <ul className="space-y-1 text-sm text-zinc-300">
              {analysis.notes.map((n) => (
                <li key={n}>{n}</li>
              ))}
            </ul>
          </div>
        )}
      </div>
    </div>
    </LifeChatSplit>
  );
}

const POT_COLORS = [
  "bg-blue-500",
  "bg-emerald-500",
  "bg-amber-500",
  "bg-violet-500",
  "bg-rose-400",
  "bg-cyan-500",
];

function Pots({
  pots,
  totalCents,
  onChange,
}: {
  pots: MoneyPotShare[];
  totalCents: number;
  onChange: () => void;
}) {
  const [adding, setAdding] = useState(false);
  const [name, setName] = useState("");
  const [pounds, setPounds] = useState("");

  async function save() {
    const amount = Number(pounds);
    if (!name.trim() || !Number.isFinite(amount) || amount < 0) return;
    await moneyUpsertPot({
      id: "",
      name: name.trim(),
      slug: "",
      balance_cents: Math.round(amount * 100),
      created_at: 0,
      updated_at: 0,
    });
    setName("");
    setPounds("");
    setAdding(false);
    onChange();
  }

  return (
    <section className="rounded-2xl border border-zinc-800">
      <div className="flex items-center justify-between border-b border-zinc-800 px-4 py-2">
        <div className="flex items-center gap-2">
          <PiggyBank size={16} className="text-emerald-400" />
          <h3 className="text-xs uppercase tracking-wider text-zinc-500">Savings pots</h3>
          {totalCents > 0 && (
            <span className="text-xs text-zinc-500">{gbp(totalCents)}</span>
          )}
        </div>
        {!adding && (
          <button type="button" onClick={() => setAdding(true)} className="text-xs text-blue-400">
            Add
          </button>
        )}
      </div>
      {pots.length > 0 && totalCents > 0 && (
        <div className="flex h-2 overflow-hidden bg-zinc-900">
          {pots.map((pot, i) => (
            <div
              key={pot.id}
              title={`${pot.name} ${gbp(pot.balance_cents)} (${pot.pct.toFixed(0)}%)`}
              className={POT_COLORS[i % POT_COLORS.length]}
              style={{ width: `${Math.max(pot.pct, 0)}%` }}
            />
          ))}
        </div>
      )}
      {adding && (
        <div className="flex flex-wrap items-center gap-2 border-b border-zinc-800 px-4 py-3">
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="Holiday, emergency…"
            className="min-w-0 flex-1 bg-transparent text-sm text-zinc-200 outline-none"
          />
          <label className="flex items-center gap-1 text-sm text-zinc-500">
            £
            <input
              type="text"
              inputMode="decimal"
              value={pounds}
              onChange={(e) => setPounds(e.target.value)}
              placeholder="0.00"
              className="w-20 bg-transparent text-sm text-zinc-200 outline-none"
            />
          </label>
          <button
            type="button"
            onClick={() => setAdding(false)}
            className="rounded-lg px-2 py-1 text-xs text-zinc-500 hover:text-zinc-200"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={() => void save()}
            className="rounded-lg bg-blue-500 px-2.5 py-1 text-xs text-white"
          >
            Save
          </button>
        </div>
      )}
      <div className="divide-y divide-zinc-800/70">
        {pots.map((pot, i) => (
          <div key={pot.id} className="flex min-w-0 items-center gap-2 px-4 py-2">
            <span className={`h-2 w-2 shrink-0 rounded-full ${POT_COLORS[i % POT_COLORS.length]}`} />
            <input
              defaultValue={pot.name}
              onBlur={(e) => {
                const next = e.target.value.trim();
                if (!next || next === pot.name) return;
                void moneyUpsertPot({
                  id: pot.id,
                  name: next,
                  slug: "",
                  balance_cents: pot.balance_cents,
                  created_at: 0,
                  updated_at: 0,
                }).then(onChange);
              }}
              className="min-w-0 flex-1 truncate bg-transparent text-sm text-zinc-200 outline-none"
            />
            <span className="hidden w-10 shrink-0 text-right text-xs text-zinc-500 @[22rem]:block">
              {pot.pct.toFixed(0)}%
            </span>
            <label className="flex w-[4.75rem] shrink-0 items-center gap-0.5 text-sm tabular-nums text-zinc-500">
              £
              <input
                type="text"
                inputMode="decimal"
                defaultValue={(pot.balance_cents / 100).toFixed(2)}
                onBlur={(e) => {
                  const cents = Math.round(Number(e.target.value) * 100);
                  if (!Number.isFinite(cents) || cents === pot.balance_cents) return;
                  void moneyUpsertPot({
                    id: pot.id,
                    name: pot.name,
                    slug: "",
                    balance_cents: cents,
                    created_at: 0,
                    updated_at: 0,
                  }).then(onChange);
                }}
                className="w-14 min-w-0 bg-transparent text-sm tabular-nums text-zinc-200 outline-none"
              />
            </label>
            <button
              type="button"
              title="Delete"
              aria-label="Delete"
              onClick={() => moneyDeletePot(pot.id).then(onChange)}
              className="rounded-md p-1 text-zinc-600 hover:text-rose-400"
            >
              <Trash size={14} />
            </button>
          </div>
        ))}
        {pots.length === 0 && !adding && (
          <p className="px-4 py-6 text-center text-xs text-pretty text-zinc-600">
            No pots yet — say “holiday pot £200, emergency £500”
          </p>
        )}
      </div>
    </section>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0 overflow-hidden rounded-2xl bg-zinc-800/60 px-3 py-3">
      <p className="truncate text-[11px] uppercase tracking-wider text-zinc-500">{label}</p>
      <p className="mt-1 truncate text-base font-semibold tabular-nums tracking-tight text-zinc-100 @[22rem]:text-lg">
        {value}
      </p>
    </div>
  );
}

function Ledger({
  title,
  kind,
  year,
  month,
  rows,
  onChange,
}: {
  title: string;
  kind: "income" | "expense";
  year: number;
  month: number;
  rows: MoneyEntry[];
  onChange: () => void;
}) {
  const [adding, setAdding] = useState(false);

  return (
    <section className="rounded-2xl border border-zinc-800">
      <div className="flex items-center justify-between border-b border-zinc-800 px-4 py-2">
        <h3 className="text-xs uppercase tracking-wider text-zinc-500">{title}</h3>
        {!adding && (
          <button
            type="button"
            onClick={() => setAdding(true)}
            className="text-xs text-blue-400"
          >
            Add
          </button>
        )}
      </div>
      {adding && (
        <AddForm
          kind={kind}
          year={year}
          month={month}
          onCancel={() => setAdding(false)}
          onSaved={() => {
            setAdding(false);
            onChange();
          }}
        />
      )}
      <div className="divide-y divide-zinc-800/70">
        {rows.map((row) => (
          <EntryRow key={row.id} row={row} onChange={onChange} />
        ))}
        {rows.length === 0 && !adding && (
          <p className="px-4 py-6 text-center text-xs text-zinc-600">None yet</p>
        )}
      </div>
    </section>
  );
}

function AddForm({
  kind,
  year,
  month,
  onCancel,
  onSaved,
}: {
  kind: "income" | "expense";
  year: number;
  month: number;
  onCancel: () => void;
  onSaved: () => void;
}) {
  const cats = catsFor(kind);
  const maxDay = daysInMonth(year, month);
  const now = new Date();
  const defaultDay =
    now.getFullYear() === year && now.getMonth() + 1 === month
      ? Math.min(now.getDate(), maxDay)
      : 1;
  const [day, setDay] = useState(defaultDay);
  const [category, setCategory] = useState(cats[0].id);
  const [what, setWhat] = useState("");
  const [pounds, setPounds] = useState("");

  async function save() {
    const amount = Number(pounds);
    if (!Number.isFinite(amount) || amount <= 0) return;
    await moneyUpsert({
      id: "",
      kind,
      date: `${year}-${pad(month)}-${pad(day)}`,
      description: what.trim() || catMeta(kind, category).label,
      category,
      amount_cents: Math.round(amount * 100),
      year,
      month,
      created_at: 0,
      updated_at: 0,
    });
    onSaved();
  }

  return (
    <div className="space-y-2 border-b border-zinc-800 px-4 py-3">
      <div className="flex flex-wrap items-center gap-2">
        <select
          value={day}
          onChange={(e) => setDay(Number(e.target.value))}
          title="Day"
          className={selectClass}
        >
          {Array.from({ length: maxDay }, (_, i) => i + 1).map((d) => (
            <option key={d} value={d}>
              {d} {MONTHS[month - 1].slice(0, 3)}
            </option>
          ))}
        </select>
        <div className="flex flex-wrap items-center gap-0.5" role="group" aria-label="Category">
          {cats.map(({ id, label, Icon }) => {
            const selected = category === id;
            return (
              <button
                key={id}
                type="button"
                title={label}
                aria-label={label}
                aria-pressed={selected}
                onClick={() => setCategory(id)}
                className={`rounded-md p-1 ${
                  selected ? "text-blue-400" : "text-zinc-600 hover:text-zinc-300"
                }`}
              >
                <Icon size={16} weight={selected ? "fill" : "regular"} />
              </button>
            );
          })}
        </div>
      </div>
      <div className="flex flex-wrap items-center gap-2">
        <input
          value={what}
          onChange={(e) => setWhat(e.target.value)}
          placeholder="What"
          className="min-w-0 flex-1 rounded-lg bg-transparent px-0 py-1 text-sm text-zinc-200 outline-none"
        />
        <label className="flex items-center gap-1 text-sm text-zinc-500">
          £
          <input
            type="text"
            inputMode="decimal"
            value={pounds}
            onChange={(e) => setPounds(e.target.value)}
            placeholder="0.00"
            className="w-20 bg-transparent text-sm text-zinc-200 outline-none"
          />
        </label>
        <button
          type="button"
          onClick={onCancel}
          className="rounded-lg px-2 py-1 text-xs text-zinc-500 hover:text-zinc-200"
        >
          Cancel
        </button>
        <button
          type="button"
          onClick={() => void save()}
          className="rounded-lg bg-blue-500 px-2.5 py-1 text-xs text-white"
        >
          Save
        </button>
      </div>
    </div>
  );
}

function EntryRow({ row, onChange }: { row: MoneyEntry; onChange: () => void }) {
  const { Icon, label } = catMeta(row.kind, row.category);
  return (
    <div className="flex min-w-0 items-center gap-2 px-4 py-2">
      <span title={label} className="shrink-0 text-zinc-400">
        <Icon size={16} weight="fill" />
      </span>
      <input
        defaultValue={row.description}
        onBlur={(e) => moneyUpsert({ ...row, description: e.target.value }).then(onChange)}
        className="min-w-0 flex-1 truncate bg-transparent text-sm text-zinc-200 outline-none"
      />
      <span className="hidden shrink-0 text-xs text-zinc-500 @[22rem]:inline">{shortDate(row.date)}</span>
      <label className="flex w-[4.75rem] shrink-0 items-center gap-0.5 text-sm tabular-nums text-zinc-500">
        £
        <input
          type="text"
          inputMode="decimal"
          defaultValue={(row.amount_cents / 100).toFixed(2)}
          onBlur={(e) =>
            moneyUpsert({
              ...row,
              amount_cents: Math.round(Number(e.target.value) * 100),
            }).then(onChange)
          }
          className="w-14 min-w-0 bg-transparent text-sm tabular-nums text-zinc-200 outline-none"
        />
      </label>
      <button
        type="button"
        title="Delete"
        aria-label="Delete"
        onClick={() => moneyDelete(row.id).then(onChange)}
        className="rounded-md p-1 text-zinc-600 hover:text-rose-400"
      >
        <Trash size={14} />
      </button>
    </div>
  );
}
