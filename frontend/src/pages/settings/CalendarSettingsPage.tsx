import { useMemo, useState, type ReactNode } from "react";
import { Link } from "react-router-dom";
import {
  Briefcase,
  CalendarBlank,
  CaretLeft,
  CaretRight,
  Clock,
  Moon,
  Plus,
  Trash,
  UserCircle,
  UsersThree,
} from "@phosphor-icons/react";
import { SectionHead } from "../../components/ui/SectionHead";
import { Surface } from "../../components/ui/Surface";
import { Tag } from "../../components/ui/Tag";
import { cn } from "../../lib/cn";
import {
  PlaceholderToggle,
  SegmentedChoice,
  SettingsChip,
  SettingsCta,
  SettingsHint,
  SettingsIconBtn,
  SettingsLabel,
  SettingsMasonry,
  SettingsPageHead,
  SettingsShiftRow,
  SettingsWeekDay,
} from "./settingsUi";

const WEEKDAYS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

type WorkMode = "full_time" | "part_time";

type WorkShift = {
  id: string;
  date: string;
  start: string;
  end: string;
  place: string;
};

type WorkSchedule = {
  enabled: boolean;
  mode: WorkMode;
  start: string;
  end: string;
  days: boolean[];
  shifts: WorkShift[];
};

const FULL_TIME = {
  start: "09:00",
  end: "17:00",
  days: [true, true, true, true, true, false, false],
};

function newId() {
  return `shift-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;
}

function toDateKey(d: Date) {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

function startOfWeek(d: Date) {
  const copy = new Date(d);
  copy.setHours(12, 0, 0, 0);
  const day = copy.getDay();
  copy.setDate(copy.getDate() + (day === 0 ? -6 : 1 - day));
  return copy;
}

function addDays(d: Date, n: number) {
  const copy = new Date(d);
  copy.setDate(copy.getDate() + n);
  return copy;
}

function formatShortDate(key: string) {
  const [y, m, day] = key.split("-").map(Number);
  return new Date(y, m - 1, day).toLocaleDateString(undefined, {
    weekday: "short",
    month: "short",
    day: "numeric",
  });
}

function applyMode(prev: WorkSchedule, mode: WorkMode): WorkSchedule {
  return mode === "full_time" ? { ...prev, mode, ...FULL_TIME } : { ...prev, mode };
}

function FullTimeEditor({
  schedule,
  onChange,
}: {
  schedule: WorkSchedule;
  onChange: (next: WorkSchedule) => void;
}) {
  return (
    <>
      <div className="settings-time-grid">
        <SettingsLabel>
          Start
          <input
            type="time"
            className="field"
            value={schedule.start}
            disabled={!schedule.enabled}
            onChange={(e) => onChange({ ...schedule, start: e.target.value })}
          />
        </SettingsLabel>
        <SettingsLabel>
          End
          <input
            type="time"
            className="field"
            value={schedule.end}
            disabled={!schedule.enabled}
            onChange={(e) => onChange({ ...schedule, end: e.target.value })}
          />
        </SettingsLabel>
      </div>
      <p className="mt-3 mb-1.5 settings-meta">Active days</p>
      <div className="settings-chips">
        {WEEKDAYS.map((d, i) => (
          <SettingsChip
            key={d}
            on={schedule.days[i]}
            disabled={!schedule.enabled}
            onClick={() =>
              onChange({
                ...schedule,
                days: schedule.days.map((v, idx) => (idx === i ? !v : v)),
              })
            }
          >
            {d}
          </SettingsChip>
        ))}
      </div>
    </>
  );
}

function PartTimeShiftPlanner({
  schedule,
  onChange,
}: {
  schedule: WorkSchedule;
  onChange: (next: WorkSchedule) => void;
}) {
  const today = useMemo(() => new Date(), []);
  const [weekAnchor, setWeekAnchor] = useState(() => startOfWeek(today));
  const [selectedKey, setSelectedKey] = useState(() => toDateKey(today));
  const [place, setPlace] = useState("");
  const [start, setStart] = useState("09:00");
  const [end, setEnd] = useState("13:00");

  const weekDays = useMemo(
    () => Array.from({ length: 7 }, (_, i) => addDays(weekAnchor, i)),
    [weekAnchor],
  );

  const weekLabel = useMemo(() => {
    const opts: Intl.DateTimeFormatOptions = { month: "short", day: "numeric" };
    return `${weekDays[0].toLocaleDateString(undefined, opts)} – ${weekDays[6].toLocaleDateString(undefined, opts)}`;
  }, [weekDays]);

  const shiftsByDate = useMemo(() => {
    const map = new Map<string, WorkShift[]>();
    for (const s of schedule.shifts) {
      const list = map.get(s.date) ?? [];
      list.push(s);
      map.set(s.date, list);
    }
    for (const list of map.values()) list.sort((a, b) => a.start.localeCompare(b.start));
    return map;
  }, [schedule.shifts]);

  const selectedShifts = shiftsByDate.get(selectedKey) ?? [];
  const disabled = !schedule.enabled;
  const todayKey = toDateKey(today);

  function addShift() {
    if (disabled) return;
    onChange({
      ...schedule,
      shifts: [
        ...schedule.shifts,
        { id: newId(), date: selectedKey, start, end, place: place.trim() || "Work" },
      ],
    });
    setPlace("");
  }

  const otherWeeks = schedule.shifts.filter(
    (s) => !weekDays.some((d) => toDateKey(d) === s.date),
  ).length;

  return (
    <div className={cn("mt-3", disabled && "settings-dim")}>
      <p className="mb-1.5 settings-meta">
        Tap a day, then add each shift with place and times — shifts can differ every day.
      </p>

      <div className="mb-2 flex items-center justify-between gap-2">
        <SettingsIconBtn
          aria-label="Previous week"
          disabled={disabled}
          onClick={() => setWeekAnchor((w) => addDays(w, -7))}
        >
          <CaretLeft size={16} weight="bold" />
        </SettingsIconBtn>
        <span className="text-xs font-medium text-ink">{weekLabel}</span>
        <SettingsIconBtn
          aria-label="Next week"
          disabled={disabled}
          onClick={() => setWeekAnchor((w) => addDays(w, 7))}
        >
          <CaretRight size={16} weight="bold" />
        </SettingsIconBtn>
      </div>

      <div className="settings-week">
        {weekDays.map((d) => {
          const key = toDateKey(d);
          return (
            <SettingsWeekDay
              key={key}
              dow={WEEKDAYS[d.getDay() === 0 ? 6 : d.getDay() - 1]}
              day={d.getDate()}
              selected={key === selectedKey}
              today={key === todayKey}
              dots={shiftsByDate.get(key)?.length ?? 0}
              disabled={disabled}
              onClick={() => setSelectedKey(key)}
            />
          );
        })}
      </div>

      <div className="settings-panel">
        <p className="m-0 mb-2 text-xs font-medium text-ink">{formatShortDate(selectedKey)}</p>
        <div className="grid gap-2">
          <SettingsLabel>
            Place / label
            <input
              type="text"
              className="field"
              placeholder="e.g. Cafe, office, client…"
              value={place}
              disabled={disabled}
              onChange={(e) => setPlace(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  addShift();
                }
              }}
            />
          </SettingsLabel>
          <div className="grid grid-cols-2 gap-2">
            <SettingsLabel>
              Start
              <input
                type="time"
                className="field"
                value={start}
                disabled={disabled}
                onChange={(e) => setStart(e.target.value)}
              />
            </SettingsLabel>
            <SettingsLabel>
              End
              <input
                type="time"
                className="field"
                value={end}
                disabled={disabled}
                onChange={(e) => setEnd(e.target.value)}
              />
            </SettingsLabel>
          </div>
          <SettingsCta disabled={disabled} onClick={addShift}>
            <Plus size={16} weight="bold" />
            Add shift
          </SettingsCta>
        </div>

        {selectedShifts.length > 0 ? (
          <ul className="m-0 mt-3 flex list-none flex-col gap-1.5 p-0">
            {selectedShifts.map((s) => (
              <SettingsShiftRow
                key={s.id}
                place={s.place}
                range={`${s.start} – ${s.end}`}
                disabled={disabled}
                onRemove={() =>
                  onChange({ ...schedule, shifts: schedule.shifts.filter((x) => x.id !== s.id) })
                }
                removeIcon={<Trash size={14} />}
              />
            ))}
          </ul>
        ) : (
          <p className="mt-3 mb-0 settings-meta">No shifts on this day yet.</p>
        )}
      </div>

      {schedule.shifts.length > 0 ? (
        <SettingsHint>
          {schedule.shifts.length} shift{schedule.shifts.length === 1 ? "" : "s"} saved in preview
          {otherWeeks > 0 ? ` · ${otherWeeks} on other weeks` : ""}
        </SettingsHint>
      ) : null}
    </div>
  );
}

function WorkScheduleCard({
  title,
  icon,
  schedule,
  onChange,
}: {
  title: string;
  icon: ReactNode;
  schedule: WorkSchedule;
  onChange: (next: WorkSchedule) => void;
}) {
  return (
    <Surface>
      <SectionHead icon={icon} title={title} action={<Tag>Coming soon</Tag>} />
      <PlaceholderToggle
        checked={schedule.enabled}
        onChange={(enabled) => onChange({ ...schedule, enabled })}
        label="Protect work hours"
        hint="Preview — will sync as Busy hours later"
      />
      <div className="mt-3">
        <p className="mb-1.5 settings-meta">Hours type</p>
        <SegmentedChoice
          ariaLabel={`${title} hours type`}
          disabled={!schedule.enabled}
          value={schedule.mode}
          onChange={(mode) => onChange(applyMode(schedule, mode))}
          options={[
            { value: "full_time", label: "Full time" },
            { value: "part_time", label: "Part time" },
          ]}
        />
        <SettingsHint>
          {schedule.mode === "full_time"
            ? "Same hours most weekdays — edit below"
            : "Irregular shifts — click days on the mini calendar"}
        </SettingsHint>
      </div>
      {schedule.mode === "full_time" ? (
        <FullTimeEditor schedule={schedule} onChange={onChange} />
      ) : (
        <PartTimeShiftPlanner schedule={schedule} onChange={onChange} />
      )}
    </Surface>
  );
}

export function CalendarSettingsPage() {
  const [yours, setYours] = useState<WorkSchedule>({
    enabled: true,
    mode: "full_time",
    ...FULL_TIME,
    shifts: [],
  });
  const [partner, setPartner] = useState<WorkSchedule>({
    enabled: true,
    mode: "part_time",
    start: "09:00",
    end: "13:00",
    days: [false, false, false, false, false, false, false],
    shifts: [],
  });
  const [sleepEnabled, setSleepEnabled] = useState(true);
  const [skipWeekends, setSkipWeekends] = useState(true);

  return (
    <>
      <SettingsPageHead
        title="Calendar settings"
        subtitle="Full-time fixed hours or a part-time shift planner — placeholders for calendar sync later."
        action={<Tag tone="warn">Coming soon</Tag>}
      />

      <SettingsMasonry>
        <WorkScheduleCard
          title="Your work hours"
          icon={<UserCircle size={16} weight="duotone" />}
          schedule={yours}
          onChange={setYours}
        />
        <WorkScheduleCard
          title="Partner work hours"
          icon={<UsersThree size={16} weight="duotone" />}
          schedule={partner}
          onChange={setPartner}
        />

        <Surface>
          <SectionHead
            icon={<Moon size={16} weight="duotone" />}
            title="Sleep & rest"
            action={<Tag>Coming soon</Tag>}
          />
          <PlaceholderToggle
            checked={sleepEnabled}
            onChange={setSleepEnabled}
            label="Protect sleep blocks"
            hint="Preview — evening / early morning"
          />
          <PlaceholderToggle
            checked={skipWeekends}
            onChange={setSkipWeekends}
            label="Lighter weekends"
            hint="Preview — prefer weekday proposals"
          />
        </Surface>

        <Surface>
          <SectionHead
            icon={<Clock size={16} weight="duotone" />}
            title="Proposal window"
            action={<Tag>Coming soon</Tag>}
          />
          <SettingsLabel>
            Prefer sessions after
            <input type="time" className="field" defaultValue="17:30" disabled />
          </SettingsLabel>
          <SettingsHint>
            Placeholder for calendar sync later — proposes already prefer evenings.
          </SettingsHint>
        </Surface>

        <Surface>
          <SectionHead icon={<Briefcase size={16} weight="duotone" />} title="How this will sync" />
          <p className="m-0 mb-2 text-sm text-muted">
            Full time stays a fixed block. Part time uses each typed shift (place + times) as Busy
            hours when calendar sync lands.
          </p>
          <p className="m-0 mb-0 settings-meta">Not saved yet — local preview only.</p>
        </Surface>

        <Surface>
          <SectionHead icon={<CalendarBlank size={16} weight="duotone" />} title="Open calendar" />
          <p className="m-0 mb-3 text-sm text-muted">
            Categories, busy hours, and day filters live on the calendar panel today.
          </p>
          <Link to="/calendar" className="settings-cta">
            Go to Calendar
          </Link>
        </Surface>
      </SettingsMasonry>
    </>
  );
}
