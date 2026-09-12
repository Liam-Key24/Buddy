import { useState, type ComponentType } from "react";
import {
  Barbell,
  CaretDown,
  CaretUp,
  Fire,
  Lightning,
  Scales,
  Trash,
} from "@phosphor-icons/react";
import { LifeChatSplit } from "../components/LifeChatSplit";
import { ToolWorkspace } from "../components/ToolWorkspace";
import { DatedModeLog } from "../components/life/DatedModeLog";
import { WorkspaceNavItem, WorkspacePaneHeader } from "../components/life/WorkspaceChrome";
import { useLifePage } from "../hooks/useLifePage";
import { todayIso } from "../lib/dates";
import {
  fitnessDeleteClimb,
  fitnessDeleteFood,
  fitnessDeleteFridge,
  fitnessDeleteWorkout,
  fitnessListClimbs,
  fitnessListFood,
  fitnessListFridge,
  fitnessListWeight,
  fitnessListWorkouts,
  fitnessOverview,
  fitnessSaveWorkout,
  fitnessSuggestMeals,
  fitnessUpsertClimb,
  fitnessUpsertFood,
  fitnessUpsertFridge,
  fitnessUpsertWeight,
  type Climb,
  type FitnessOverview,
  type FoodEntry,
  type FridgeItem,
  type SuggestedMeal,
  type WeightEntry,
  type Workout,
} from "../lib/lifeApi";

const SECTIONS = ["Fitness", "Food", "Workout"] as const;
type Section = (typeof SECTIONS)[number];

export function Fitness() {
  const [section, setSection] = useState<Section>("Fitness");
  const [overview, setOverview] = useState<FitnessOverview | null>(null);
  const [foods, setFoods] = useState<FoodEntry[]>([]);
  const [fridge, setFridge] = useState<FridgeItem[]>([]);
  const [meals, setMeals] = useState<SuggestedMeal[]>([]);
  const [weights, setWeights] = useState<WeightEntry[]>([]);
  const [climbs, setClimbs] = useState<Climb[]>([]);
  const [workouts, setWorkouts] = useState<Workout[]>([]);

  async function reload() {
    const [o, f, fr, m, w, c, wo] = await Promise.all([
      fitnessOverview(),
      fitnessListFood(),
      fitnessListFridge(),
      fitnessSuggestMeals(),
      fitnessListWeight(),
      fitnessListClimbs(),
      fitnessListWorkouts(),
    ]);
    setOverview(o);
    setFoods(f);
    setFridge(fr);
    setMeals(m);
    setWeights(w);
    setClimbs(c);
    setWorkouts(wo);
  }

  useLifePage({
    event: "fitness-updated",
    load: reload,
    context: `Page: fitness. Section ${section}. Date ${todayIso()}. Read with fitness.look (food|workouts|weight|climbs|prs|fridge). Log food via fitness.log_food (estimate macros). Log sessions via fitness.log_workout with sets.`,
  });

  return (
    <LifeChatSplit>
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <ToolWorkspace
        sidebar={
          <>
            {SECTIONS.map((s) => (
              <WorkspaceNavItem key={s} active={section === s} onClick={() => setSection(s)}>
                {s}
              </WorkspaceNavItem>
            ))}
          </>
        }
      >
        <WorkspacePaneHeader Icon={Barbell} title={section} />
        <div className="min-h-0 flex-1 overflow-y-auto pr-1">
          {section === "Fitness" && (
            <div className="space-y-6">
              <div className="grid grid-cols-2 gap-3">
                <WeightCard weights={weights} onChange={reload} />
                <Stat Icon={Barbell} label="Workouts" value={String(overview?.workouts_this_week ?? 0)} />
                <Stat Icon={Fire} label="Calories" value={String(Math.round(overview?.calories_eaten ?? 0))} />
                <Stat Icon={Lightning} label="Remaining" value={`${Math.round(overview?.remaining ?? 0)}`} />
              </div>
              {weights.length >= 2 && (
                <Sparkline values={[...weights].reverse().map((w) => w.kg)} />
              )}
              <section className="space-y-2">
                <h4 className="text-xs uppercase tracking-wider text-zinc-500">Recipes</h4>
                {meals.length === 0 && (
                  <p className="text-xs text-zinc-600">No suggestions yet</p>
                )}
                {meals.map((m) => (
                  <div key={m.name} className="rounded-xl border border-zinc-800 p-3">
                    <p className="text-sm text-zinc-100">{m.name} · {Math.round(m.calories)} kcal</p>
                    <p className="mt-1 text-xs text-zinc-500">{m.reason}</p>
                  </div>
                ))}
              </section>
            </div>
          )}
          {section === "Food" && (
            <FoodLog foods={foods} fridge={fridge} onChange={reload} />
          )}
          {section === "Workout" && (
            <WorkoutLog climbs={climbs} workouts={workouts} onChange={reload} />
          )}
        </div>
      </ToolWorkspace>
    </div>
    </LifeChatSplit>
  );
}

function Stat({
  label,
  value,
  Icon,
}: {
  label: string;
  value: string;
  Icon?: ComponentType<{ size?: number; className?: string; weight?: "fill" | "regular" }>;
}) {
  return (
    <div className="flex items-center gap-3 rounded-2xl bg-zinc-800/50 p-4">
      {Icon && <Icon size={22} weight="fill" className="shrink-0 text-blue-400" />}
      <div className="min-w-0">
        <p className="text-xs text-zinc-500">{label}</p>
        <p className="truncate text-lg font-semibold text-zinc-100">{value}</p>
      </div>
    </div>
  );
}

function Sparkline({ values }: { values: number[] }) {
  if (values.length < 2) return null;
  const min = Math.min(...values);
  const max = Math.max(...values);
  const w = 400;
  const h = 64;
  const pts = values.map((v, i) => {
    const x = (i / (values.length - 1)) * w;
    const y = h - ((v - min) / (max - min || 1)) * (h - 8) - 4;
    return `${x},${y}`;
  });
  return (
    <svg viewBox={`0 0 ${w} ${h}`} className="h-16 w-full text-blue-400">
      <polyline fill="none" stroke="currentColor" strokeWidth="2" points={pts.join(" ")} />
    </svg>
  );
}

function dayFromTs(ms: number) {
  const d = new Date(ms);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

function middayMs(iso: string) {
  return new Date(`${iso}T12:00:00`).getTime();
}

type FoodMode = "food" | "fridge";
type KitchenItem =
  | { kind: "food"; at: number; date: string; food: FoodEntry }
  | { kind: "fridge"; at: number; date: string; item: FridgeItem };

function kitchenItems(foods: FoodEntry[], fridge: FridgeItem[]): KitchenItem[] {
  return [
    ...foods.map((food) => ({ kind: "food" as const, at: food.created_at, date: food.date, food })),
    ...fridge.map((item) => ({
      kind: "fridge" as const,
      at: item.created_at,
      date: dayFromTs(item.created_at),
      item,
    })),
  ];
}

function FoodLog({
  foods,
  fridge,
  onChange,
}: {
  foods: FoodEntry[];
  fridge: FridgeItem[];
  onChange: () => void;
}) {
  const [mode, setMode] = useState<FoodMode>("food");

  async function addFood(date: string) {
    await fitnessUpsertFood({
      id: "",
      name: "",
      quantity: 1,
      unit: "serving",
      calories: 0,
      protein: 0,
      carbs: 0,
      fat: 0,
      date,
      meal_type: "meal",
      created_at: middayMs(date),
    });
    onChange();
  }

  async function addFridge(date: string) {
    await fitnessUpsertFridge({
      id: "",
      name: "",
      quantity: 1,
      unit: "item",
      category: "other",
      expiry_date: null,
      created_at: middayMs(date),
      updated_at: 0,
    });
    onChange();
  }

  return (
    <DatedModeLog
      modes={[
        { id: "food", label: "Food" },
        { id: "fridge", label: "Fridge" },
      ]}
      mode={mode}
      onModeChange={(id) => setMode(id as FoodMode)}
      onAdd={(date) => void (mode === "food" ? addFood(date) : addFridge(date))}
      items={kitchenItems(foods, fridge)}
      itemMatchesMode={(item, m) => item.kind === m}
      itemKey={(item) => (item.kind === "food" ? item.food.id : item.item.id)}
      renderItem={(item) =>
        item.kind === "food" ? (
          <FoodCard food={item.food} onChange={onChange} />
        ) : (
          <FridgeCard item={item.item} onChange={onChange} />
        )
      }
    />
  );
}

function FoodCard({ food, onChange }: { food: FoodEntry; onChange: () => void }) {
  return (
    <div className="rounded-xl border border-zinc-800 p-3">
      <div className="mb-2 flex items-center gap-2">
        <input
          defaultValue={food.name}
          placeholder="Food"
          onBlur={(e) => fitnessUpsertFood({ ...food, name: e.target.value }).then(onChange)}
          className="min-w-0 flex-1 bg-transparent text-sm text-zinc-100 outline-none"
        />
        <button
          type="button"
          title="Remove"
          aria-label="Remove"
          onClick={() => fitnessDeleteFood(food.id).then(onChange)}
          className="rounded-md p-1 text-zinc-600 hover:text-rose-400"
        >
          <Trash size={14} />
        </button>
      </div>
      <label className="flex items-center gap-1 text-sm text-zinc-500">
        <input
          type="text"
          inputMode="decimal"
          defaultValue={food.calories || ""}
          placeholder="0"
          onBlur={(e) =>
            fitnessUpsertFood({ ...food, calories: Number(e.target.value) || 0 }).then(onChange)
          }
          className="w-16 bg-transparent text-sm text-zinc-100 outline-none"
        />
        kcal
      </label>
    </div>
  );
}

function FridgeCard({ item, onChange }: { item: FridgeItem; onChange: () => void }) {
  return (
    <div className="rounded-xl border border-zinc-800 p-3">
      <div className="mb-2 flex items-center gap-2">
        <input
          defaultValue={item.name}
          placeholder="Item"
          onBlur={(e) => fitnessUpsertFridge({ ...item, name: e.target.value }).then(onChange)}
          className="min-w-0 flex-1 bg-transparent text-sm text-zinc-100 outline-none"
        />
        <button
          type="button"
          title="Remove"
          aria-label="Remove"
          onClick={() => fitnessDeleteFridge(item.id).then(onChange)}
          className="rounded-md p-1 text-zinc-600 hover:text-rose-400"
        >
          <Trash size={14} />
        </button>
      </div>
      <Counter
        label="Qty"
        value={Math.max(1, item.quantity)}
        onChange={(n) => fitnessUpsertFridge({ ...item, quantity: n }).then(onChange)}
      />
    </div>
  );
}

function WeightCard({
  weights,
  onChange,
}: {
  weights: WeightEntry[];
  onChange: () => void;
}) {
  const latest = weights[0]?.kg;
  const [kg, setKg] = useState(latest != null ? String(latest) : "");

  async function log() {
    const n = Number(kg);
    if (!Number.isFinite(n) || n <= 0) return;
    if (latest != null && n === latest) return;
    await fitnessUpsertWeight({ id: "", date: todayIso(), kg: n, notes: null, created_at: 0 });
    onChange();
  }

  return (
    <div className="flex items-center gap-3 rounded-2xl bg-zinc-800/50 p-4">
      <Scales size={22} weight="fill" className="shrink-0 text-blue-400" />
      <input
        type="text"
        inputMode="decimal"
        value={kg}
        onChange={(e) => setKg(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            void log();
          }
        }}
        onBlur={() => void log()}
        placeholder="kg"
        className="min-w-0 flex-1 bg-transparent text-lg font-semibold text-zinc-100 outline-none"
      />
    </div>
  );
}

type TrackKind = "climbing" | "workout";
type TrackItem =
  | { kind: "workout"; at: number; date: string; workout: Workout }
  | { kind: "climb"; at: number; date: string; climb: Climb };

function toItems(climbs: Climb[], workouts: Workout[]): TrackItem[] {
  return [
    ...workouts.map((w) => ({ kind: "workout" as const, at: w.created_at, date: w.date, workout: w })),
    ...climbs.map((c) => ({ kind: "climb" as const, at: c.created_at, date: c.date, climb: c })),
  ];
}

function WorkoutLog({
  climbs,
  workouts,
  onChange,
}: {
  climbs: Climb[];
  workouts: Workout[];
  onChange: () => void;
}) {
  const [mode, setMode] = useState<TrackKind>("workout");

  async function addWorkout(date: string) {
    await fitnessSaveWorkout({
      id: "",
      name: "",
      date,
      notes: null,
      duration_minutes: null,
      created_at: 0,
      sets: makeSets("", 3, 8),
    });
    onChange();
  }

  async function addClimb(date: string) {
    await fitnessUpsertClimb({
      id: "",
      name: "Send",
      grade: "V3",
      date,
      location: null,
      attempts: 1,
      sent: true,
      project: false,
      style: null,
      notes: null,
      created_at: 0,
    });
    onChange();
  }

  return (
    <DatedModeLog
      modes={[
        { id: "climbing", label: "Climbing" },
        { id: "workout", label: "Workout" },
      ]}
      mode={mode}
      onModeChange={(id) => setMode(id as TrackKind)}
      onAdd={(date) => void (mode === "climbing" ? addClimb(date) : addWorkout(date))}
      items={toItems(climbs, workouts)}
      itemMatchesMode={(item, m) => (m === "climbing" ? item.kind === "climb" : item.kind === "workout")}
      itemKey={(item) => (item.kind === "workout" ? item.workout.id : item.climb.id)}
      renderItem={(item) =>
        item.kind === "workout" ? (
          <SimpleWorkoutCard workout={item.workout} onChange={onChange} />
        ) : (
          <ClimbCard climb={item.climb} onChange={onChange} />
        )
      }
    />
  );
}

function makeSets(name: string, sets: number, reps: number): Workout["sets"] {
  return Array.from({ length: Math.max(1, sets) }, (_, i) => ({
    id: "",
    workout_id: "",
    exercise: name || "Workout",
    set_index: i + 1,
    reps,
    weight: null,
    duration_seconds: null,
    rest_seconds: null,
    distance: null,
    notes: null,
  }));
}

function SimpleWorkoutCard({
  workout,
  onChange,
}: {
  workout: Workout;
  onChange: () => void;
}) {
  const sets = Math.max(1, workout.sets.length || 1);
  const reps = workout.sets[0]?.reps ?? 8;

  async function save(name: string, nextSets: number, nextReps: number) {
    await fitnessSaveWorkout({
      ...workout,
      name,
      sets: makeSets(name, nextSets, nextReps),
    });
    onChange();
  }

  return (
    <div className="rounded-xl border border-zinc-800 p-3">
      <div className="mb-3 flex items-center gap-2">
        <input
          defaultValue={workout.name}
          placeholder="Title"
          onBlur={(e) => void save(e.target.value, sets, reps)}
          className="min-w-0 flex-1 bg-transparent text-sm text-zinc-100 outline-none"
        />
        <button
          type="button"
          title="Remove"
          aria-label="Remove"
          onClick={() => fitnessDeleteWorkout(workout.id).then(onChange)}
          className="rounded-md p-1 text-zinc-600 hover:text-rose-400"
        >
          <Trash size={14} />
        </button>
      </div>
      <div className="grid grid-cols-2 gap-2">
        <Counter
          label="Sets"
          value={sets}
          onChange={(n) => void save(workout.name, n, reps)}
        />
        <Counter
          label="Reps"
          value={reps}
          onChange={(n) => void save(workout.name, sets, n)}
        />
      </div>
    </div>
  );
}

function ClimbCard({
  climb,
  onChange,
}: {
  climb: Climb;
  onChange: () => void;
}) {
  const grade = Number(String(climb.grade).replace(/^V/i, "")) || 0;

  async function setGrade(n: number) {
    await fitnessUpsertClimb({ ...climb, grade: `V${n}` });
    onChange();
  }

  return (
    <div className="rounded-xl border border-zinc-800 p-3">
      <div className="mb-2 flex items-center justify-end">
        <button
          type="button"
          title="Remove"
          aria-label="Remove"
          onClick={() => fitnessDeleteClimb(climb.id).then(onChange)}
          className="rounded-md p-1 text-zinc-600 hover:text-rose-400"
        >
          <Trash size={14} />
        </button>
      </div>
      <Counter
        label="V"
        value={grade}
        min={0}
        max={17}
        onChange={(n) => void setGrade(n)}
      />
    </div>
  );
}

function Counter({
  label,
  value,
  onChange,
  min = 1,
  max = 99,
}: {
  label: string;
  value: number;
  onChange: (n: number) => void;
  min?: number;
  max?: number;
}) {
  return (
    <div className="flex flex-col items-center rounded-xl bg-zinc-900/50 py-2">
      <span className="text-[10px] uppercase tracking-wider text-zinc-500">{label}</span>
      <button
        type="button"
        aria-label={`Increase ${label}`}
        onClick={() => onChange(Math.min(max, value + 1))}
        className="rounded-md p-0.5 text-zinc-400 hover:text-zinc-100"
      >
        <CaretUp size={16} weight="bold" />
      </button>
      <span className="text-lg font-semibold text-zinc-100">{label === "V" ? `V${value}` : value}</span>
      <button
        type="button"
        aria-label={`Decrease ${label}`}
        onClick={() => onChange(Math.max(min, value - 1))}
        className="rounded-md p-0.5 text-zinc-400 hover:text-zinc-100"
      >
        <CaretDown size={16} weight="bold" />
      </button>
    </div>
  );
}
