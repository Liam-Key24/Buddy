import { useEffect, useMemo, useRef, useState } from "react";
import FullCalendar from "@fullcalendar/react";
import dayGridPlugin from "@fullcalendar/daygrid";
import timeGridPlugin from "@fullcalendar/timegrid";
import interactionPlugin from "@fullcalendar/interaction";
import type { DayHeaderContentArg, EventContentArg } from "@fullcalendar/core";
import {
  CaretLeft,
  CaretRight,
  CheckCircle,
  Clock,
  FunnelSimple,
  Plus,
  Trash,
  X,
} from "@phosphor-icons/react";
import { ConfirmDialog } from "../components/ui/ConfirmDialog";
import { CategoryIcon } from "../components/CategoryIcon";
import { Button } from "../components/ui/Button";
import { FrostFloat } from "../components/ui/FrostFloat";
import { IconButton } from "../components/ui/IconButton";
import { Surface } from "../components/ui/Surface";
import { Tag } from "../components/ui/Tag";
import { cn } from "../lib/cn";
import {
  createCategory,
  createFixedBlock,
  createSession,
  decideProposal,
  deleteCategory,
  deleteFixedBlock,
  deleteSession,
  fetchCategories,
  fetchFixedBlocks,
  fetchSessions,
  formatSessionWhen,
  markSessionOutcome,
  notifyCalendarChanged,
  updateFixedBlock,
  type Category,
  type FixedBlock,
  type Session,
} from "../api";

type CalView = "timeGridDay" | "timeGridWeek" | "dayGridMonth";

const VIEW_TABS: Array<{ id: CalView; label: string }> = [
  { id: "timeGridDay", label: "Day" },
  { id: "timeGridWeek", label: "Week" },
  { id: "dayGridMonth", label: "Month" },
];

const COLOR_PRESETS = ["#eaf6cb", "#c5d9a0", "#9dde9a", "#a8c5a0", "#e8c56b", "#f0a0a0", "#8fb9a8"];

const WEEKDAY_LABELS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

function minutesToTime(m: number): string {
  const h = Math.floor(m / 60);
  const min = m % 60;
  return `${String(h).padStart(2, "0")}:${String(min).padStart(2, "0")}:00`;
}

function timeToMinutes(t: string): number {
  const [h, m] = t.split(":").map(Number);
  return h * 60 + (m || 0);
}

/** Python weekday (Mon=0) → FullCalendar daysOfWeek (Sun=0). */
function pythonWeekdayToFc(weekday: number): number {
  return (weekday + 1) % 7;
}

function localDateInput(d = new Date()): string {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

function localTimeInput(d = new Date(), roundHour = true): string {
  const copy = new Date(d);
  if (roundHour) {
    copy.setMinutes(0, 0, 0);
    if (d.getMinutes() > 0) copy.setHours(copy.getHours() + 1);
  }
  return `${String(copy.getHours()).padStart(2, "0")}:${String(copy.getMinutes()).padStart(2, "0")}`;
}

function toIsoLocal(date: string, time: string): string {
  return new Date(`${date}T${time}:00`).toISOString();
}

function formatRange(session: Session): string {
  const start = new Date(session.start_at);
  const end = new Date(session.end_at);
  const opts: Intl.DateTimeFormatOptions = { hour: "numeric", minute: "2-digit" };
  const a = start.toLocaleTimeString(undefined, opts).replace(":00", "");
  const b = end.toLocaleTimeString(undefined, opts).replace(":00", "");
  return `${a} – ${b}`;
}

function groupFixedBlocks(blocks: FixedBlock[]) {
  const map = new Map<
    string,
    { title: string; start_minute: number; end_minute: number; weekdays: number[]; ids: string[] }
  >();
  for (const b of blocks) {
    const key = `${b.title.toLowerCase()}|${b.start_minute}|${b.end_minute}`;
    const row = map.get(key);
    if (row) {
      row.weekdays.push(b.weekday);
      row.ids.push(b.id);
    } else {
      map.set(key, {
        title: b.title,
        start_minute: b.start_minute,
        end_minute: b.end_minute,
        weekdays: [b.weekday],
        ids: [b.id],
      });
    }
  }
  return [...map.values()].map((g) => ({
    ...g,
    weekdays: [...new Set(g.weekdays)].sort((a, b) => a - b),
  }));
}

function weekdayRangeLabel(days: number[]): string {
  if (!days.length) return "";
  if (days.length === 7) return "Every day";
  const labels = days.map((d) => WEEKDAY_LABELS[d]);
  if (days.length === 1) return labels[0];
  const consecutive = days.every((d, i) => i === 0 || d === days[i - 1] + 1);
  if (consecutive) return `${labels[0]}–${labels[labels.length - 1]}`;
  return labels.join(", ");
}

function buildMonthCells(year: number, month: number) {
  const first = new Date(year, month, 1);
  const startPad = (first.getDay() + 6) % 7; // Monday-first
  const daysInMonth = new Date(year, month + 1, 0).getDate();
  const prevDays = new Date(year, month, 0).getDate();
  const cells: Array<{ day: number; inMonth: boolean; date: Date }> = [];
  for (let i = startPad - 1; i >= 0; i--) {
    const day = prevDays - i;
    cells.push({ day, inMonth: false, date: new Date(year, month - 1, day) });
  }
  for (let d = 1; d <= daysInMonth; d++) {
    cells.push({ day: d, inMonth: true, date: new Date(year, month, d) });
  }
  while (cells.length % 7 !== 0) {
    const day = cells.length - (startPad + daysInMonth) + 1;
    cells.push({ day, inMonth: false, date: new Date(year, month + 1, day) });
  }
  return cells;
}

function sameDay(a: Date, b: Date) {
  return (
    a.getFullYear() === b.getFullYear() &&
    a.getMonth() === b.getMonth() &&
    a.getDate() === b.getDate()
  );
}

export function CalendarPage() {
  const calendarRef = useRef<FullCalendar | null>(null);
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [sessions, setSessions] = useState<Session[]>([]);
  const [categories, setCategories] = useState<Category[]>([]);
  const [enabled, setEnabled] = useState<Record<string, boolean>>({});
  const [view, setView] = useState<CalView>("timeGridWeek");
  const [selected, setSelected] = useState<Session | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [addCategoryOpen, setAddCategoryOpen] = useState(false);
  const [newName, setNewName] = useState("");
  const [newKeywords, setNewKeywords] = useState("");
  const [newColor, setNewColor] = useState(COLOR_PRESETS[0]);
  const [eventOpen, setEventOpen] = useState(false);
  const [eventTitle, setEventTitle] = useState("");
  const [eventDate, setEventDate] = useState(localDateInput);
  const [eventStart, setEventStart] = useState(() => localTimeInput());
  const [eventEnd, setEventEnd] = useState(() => {
    const d = new Date();
    d.setHours(d.getHours() + 1, 0, 0, 0);
    return localTimeInput(d, false);
  });
  const [eventCategoryId, setEventCategoryId] = useState("");
  const [pendingDelete, setPendingDelete] = useState<Session | null>(null);
  const [fixedBlocks, setFixedBlocks] = useState<FixedBlock[]>([]);
  const [fixedOpen, setFixedOpen] = useState(false);
  const [fixedTitle, setFixedTitle] = useState("");
  const [fixedWeekday, setFixedWeekday] = useState(0);
  const [fixedStart, setFixedStart] = useState("09:00");
  const [fixedEnd, setFixedEnd] = useState("17:00");
  const [editingFixedId, setEditingFixedId] = useState<string | null>(null);
  const [miniCursor, setMiniCursor] = useState(() => {
    const n = new Date();
    return { year: n.getFullYear(), month: n.getMonth() };
  });
  const [selectedDay, setSelectedDay] = useState(() => new Date());

  async function reload() {
    const [s, c, f] = await Promise.all([
      fetchSessions(),
      fetchCategories(),
      fetchFixedBlocks().catch(() => [] as FixedBlock[]),
    ]);
    setSessions(s);
    setCategories(c);
    setFixedBlocks(f);
    setEnabled((prev) => {
      const next = { ...prev };
      for (const cat of c) {
        if (next[cat.id] === undefined) next[cat.id] = true;
      }
      return next;
    });
  }

  useEffect(() => {
    reload().catch((e: Error) => setError(e.message));
    const onCalendarChanged = () => {
      reload().catch((e: Error) => setError(e.message));
    };
    window.addEventListener("buddy.calendar-changed", onCalendarChanged);
    return () => window.removeEventListener("buddy.calendar-changed", onCalendarChanged);
  }, []);

  const filtered = useMemo(() => {
    return sessions.filter((s) => {
      const id = s.category_id;
      if (!id) return true;
      return enabled[id] !== false;
    });
  }, [sessions, enabled]);

  const counts = useMemo(() => {
    const map: Record<string, number> = {};
    for (const s of sessions) {
      const id = s.category_id || "";
      if (!id) continue;
      map[id] = (map[id] || 0) + 1;
    }
    return map;
  }, [sessions]);

  const uniqueCategories = useMemo(() => {
    const seen = new Set<string>();
    return categories.filter((c) => {
      const key = c.name.trim().toLowerCase();
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });
  }, [categories]);

  const categoryTotals = useMemo(() => {
    return Math.max(
      1,
      uniqueCategories.reduce((sum, c) => sum + (counts[c.id] || 0), 0),
    );
  }, [uniqueCategories, counts]);

  const minutesUntil = (session: Session | null) => {
    if (!session) return null;
    const ms = new Date(session.start_at).getTime() - Date.now();
    if (ms <= 0) return "Now";
    const mins = Math.round(ms / 60_000);
    if (mins < 60) return `${mins} min`;
    const h = Math.floor(mins / 60);
    const m = mins % 60;
    return m ? `${h}h ${m}m` : `${h}h`;
  };

  const nextUp = useMemo(() => {
    const now = Date.now();
    return (
      [...filtered]
        .filter((s) => new Date(s.end_at).getTime() >= now - 60_000)
        .filter((s) => s.status !== "missed" && s.status !== "rejected")
        .sort((a, b) => a.start_at.localeCompare(b.start_at))[0] || null
    );
  }, [filtered]);

  const busyGroups = useMemo(() => groupFixedBlocks(fixedBlocks), [fixedBlocks]);

  const miniCells = useMemo(
    () => buildMonthCells(miniCursor.year, miniCursor.month),
    [miniCursor],
  );

  const miniLabel = useMemo(
    () =>
      new Date(miniCursor.year, miniCursor.month, 1).toLocaleDateString(undefined, {
        month: "long",
        year: "numeric",
      }),
    [miniCursor],
  );

  const events = useMemo(() => {
    const sessionEvents = filtered.map((s) => {
      const color = s.category?.color || "#c5d9a0";
      const proposed = s.status === "proposed";
      return {
        id: s.id,
        title: s.title,
        start: s.start_at,
        end: s.end_at,
        backgroundColor: proposed ? "transparent" : color,
        borderColor: "transparent",
        textColor: "#1a2422",
        classNames: [
          "evt-soft",
          proposed ? "evt-hatched" : "evt-solid",
          s.status === "missed" ? "evt-missed-soft" : "",
        ].filter(Boolean),
        extendedProps: { session: s, color },
      };
    });
    const blockEvents = fixedBlocks.map((b) => ({
      id: `fixed-${b.id}`,
      title: b.title,
      daysOfWeek: [pythonWeekdayToFc(b.weekday)],
      startTime: minutesToTime(b.start_minute),
      endTime: minutesToTime(b.end_minute),
      display: "background" as const,
      backgroundColor: "rgba(73, 90, 86, 0.28)",
      classNames: ["evt-fixed-block"],
      editable: false,
      extendedProps: { fixedBlock: b },
    }));
    return [...blockEvents, ...sessionEvents];
  }, [filtered, fixedBlocks]);


  function openEventModal() {
    const start = new Date();
    start.setMinutes(0, 0, 0);
    if (new Date().getMinutes() > 0) start.setHours(start.getHours() + 1);
    const end = new Date(start);
    end.setHours(end.getHours() + 1);
    setEventTitle("");
    setEventDate(localDateInput(start));
    setEventStart(localTimeInput(start, false));
    setEventEnd(localTimeInput(end, false));
    setEventCategoryId("");
    setEventOpen(true);
  }

  async function onAddEvent() {
    if (!eventTitle.trim() || busy) return;
    setBusy(true);
    setError(null);
    try {
      await createSession({
        title: eventTitle.trim(),
        start_at: toIsoLocal(eventDate, eventStart),
        end_at: toIsoLocal(eventDate, eventEnd),
        category_id: eventCategoryId || null,
      });
      setEventTitle("");
      setEventOpen(false);
      await reload();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Could not add event");
    } finally {
      setBusy(false);
    }
  }

  function setCalView(next: CalView) {
    setView(next);
    calendarRef.current?.getApi().changeView(next);
  }

  function renderDayHeader(arg: DayHeaderContentArg) {
    const date = arg.date;
    const weekday = date.toLocaleDateString(undefined, { weekday: "short" });
    const dayNum = date.getDate();
    const today = arg.isToday;
    return (
      <div className={`cal-day-head${today ? " is-today" : ""}`}>
        <span className="cal-day-name">{weekday}</span>
        <span className="cal-day-num">{dayNum}</span>
      </div>
    );
  }

  function renderEvent(arg: EventContentArg) {
    const session = arg.event.extendedProps.session as Session | undefined;
    const color = (arg.event.extendedProps.color as string) || "#c5d9a0";
    if (!session) return true;
    const compact = arg.view.type === "dayGridMonth";
    if (compact) {
      return (
        <div className="evt-month-chip" style={{ ["--cat-color" as string]: color }}>
          {session.title}
        </div>
      );
    }
    return (
      <div
        className={`evt-soft-inner${session.status === "proposed" ? " is-proposed" : ""}`}
        style={{ ["--cat-color" as string]: color }}
      >
        <div className="evt-soft-title">{session.title}</div>
        <div className="evt-soft-time">{formatRange(session)}</div>
      </div>
    );
  }

  async function onOutcome(outcome: "completed" | "missed") {
    if (!selected || busy) return;
    setBusy(true);
    try {
      await markSessionOutcome(selected.id, outcome);
      await reload();
      setSelected(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Update failed");
    } finally {
      setBusy(false);
    }
  }

  async function onApproveSelected() {
    if (!selected?.proposal_batch_id || busy) return;
    setBusy(true);
    setError(null);
    try {
      await decideProposal(selected.proposal_batch_id, "approve");
      notifyCalendarChanged();
      await reload();
      setSelected(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Could not approve");
    } finally {
      setBusy(false);
    }
  }

  function openFixedEditor(block?: FixedBlock) {
    if (block) {
      setEditingFixedId(block.id);
      setFixedTitle(block.title);
      setFixedWeekday(block.weekday);
      setFixedStart(minutesToTime(block.start_minute).slice(0, 5));
      setFixedEnd(minutesToTime(block.end_minute).slice(0, 5));
    } else {
      setEditingFixedId(null);
      setFixedTitle("");
      setFixedWeekday(0);
      setFixedStart("09:00");
      setFixedEnd("17:00");
    }
    setFixedOpen(true);
  }

  async function onSaveFixed() {
    if (!fixedTitle.trim() || busy) return;
    setBusy(true);
    setError(null);
    try {
      const payload = {
        title: fixedTitle.trim(),
        weekday: fixedWeekday,
        start_minute: timeToMinutes(fixedStart),
        end_minute: timeToMinutes(fixedEnd),
      };
      if (editingFixedId) {
        await updateFixedBlock(editingFixedId, payload);
      } else {
        await createFixedBlock(payload);
      }
      setFixedOpen(false);
      await reload();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Could not save fixed block");
    } finally {
      setBusy(false);
    }
  }

  async function onDeleteFixedGroup(ids: string[]) {
    if (busy || !ids.length) return;
    setBusy(true);
    try {
      for (const id of ids) {
        await deleteFixedBlock(id);
      }
      await reload();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Could not delete fixed block");
    } finally {
      setBusy(false);
    }
  }

  function jumpToDay(date: Date) {
    setSelectedDay(date);
    setMiniCursor({ year: date.getFullYear(), month: date.getMonth() });
    const api = calendarRef.current?.getApi();
    if (api) api.gotoDate(date);
  }

  async function confirmDeleteSession() {
    if (!pendingDelete || busy) return;
    const id = pendingDelete.id;
    setBusy(true);
    setError(null);
    try {
      await deleteSession(id);
      setPendingDelete(null);
      await reload();
      setSelected((prev) => (prev?.id === id ? null : prev));
    } catch (e) {
      setError(e instanceof Error ? e.message : "Could not delete event");
    } finally {
      setBusy(false);
    }
  }

  function closeAddCategory() {
    setAddCategoryOpen(false);
    setNewName("");
    setNewKeywords("");
    setNewColor(COLOR_PRESETS[0]);
  }

  async function onAddCategory() {
    if (!newName.trim() || busy) return;
    setBusy(true);
    try {
      const lower = newName.trim().toLowerCase();
      await createCategory({
        name: newName.trim(),
        color: newColor,
        keywords: newKeywords.trim(),
        icon: lower.includes("climb")
          ? "mountain"
          : lower.includes("strength") || lower.includes("workout")
            ? "barbell"
            : "circle",
      });
      closeAddCategory();
      await reload();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Could not add category");
    } finally {
      setBusy(false);
    }
  }

  async function onDeleteCategory(id: string) {
    if (busy) return;
    setBusy(true);
    try {
      await deleteCategory(id);
      await reload();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Could not delete category");
    } finally {
      setBusy(false);
    }
  }


  return (
    <section className="flex h-full min-h-0 gap-4 bg-page p-4">
      <div className="flex min-h-0 min-w-0 flex-1 flex-col">
        <header className="mb-3 flex flex-wrap items-center gap-2">
          <Button
            tone="ghost"
            className={sidebarOpen ? "xl:hidden" : ""}
            onClick={() => setSidebarOpen((v) => !v)}
            aria-label={sidebarOpen ? "Close calendar panel" : "Open calendar panel"}
          >
            <FunnelSimple size={18} />
          </Button>
          <div
            className="flex rounded-pill bg-raised-soft p-0.5"
            role="tablist"
            aria-label="Calendar view"
          >
            {VIEW_TABS.map((tab) => (
              <button
                key={tab.id}
                type="button"
                role="tab"
                aria-selected={view === tab.id}
                className={cn(
                  "rounded-pill px-3 py-1 text-sm",
                  view === tab.id ? "bg-mint text-page-deep" : "text-ink-soft",
                )}
                onClick={() => setCalView(tab.id)}
              >
                {tab.label}
              </button>
            ))}
          </div>
          <Button tone="primary" className="ml-auto" onClick={openEventModal}>
            <Plus size={16} weight="bold" /> Add
          </Button>
        </header>

        {error && (
          <div className="mb-2 rounded-card bg-danger/15 px-3 py-2 text-sm text-danger">{error}</div>
        )}

        <div className="cal-grid-wrap min-h-0 flex-1">
          <FullCalendar
            ref={calendarRef}
            plugins={[dayGridPlugin, timeGridPlugin, interactionPlugin]}
            initialView={view}
            headerToolbar={{
              left: "prev,next today",
              center: "title",
              right: "",
            }}
            firstDay={1}
            height="100%"
            expandRows
            stickyHeaderDates
            allDaySlot={false}
            slotMinTime="08:00:00"
            slotMaxTime="22:00:00"
            slotDuration="01:00:00"
            slotLabelInterval="01:00:00"
            slotLabelFormat={{ hour: "numeric", meridiem: "short" }}
            dayHeaderContent={renderDayHeader}
            events={events}
            nowIndicator
            eventContent={renderEvent}
            datesSet={() => {
              requestAnimationFrame(() => {
                const line = document.querySelector(
                  ".cal-grid-wrap .fc-timegrid-now-indicator-line",
                ) as HTMLElement | null;
                if (!line) return;
                let pill = line.querySelector(".now-pill") as HTMLElement | null;
                if (!pill) {
                  pill = document.createElement("span");
                  pill.className = "now-pill";
                  line.appendChild(pill);
                }
                pill.textContent = new Date().toLocaleTimeString(undefined, {
                  hour: "numeric",
                  minute: "2-digit",
                });
              });
            }}
            eventClick={(info) => {
              const session = info.event.extendedProps.session as Session | undefined;
              if (session) setSelected(session);
            }}
          />
        </div>

        {selected && (
          <FrostFloat className="mt-3 p-3">
            <div className="flex items-start gap-2">
              <Tag
                icon={
                  <CategoryIcon
                    name={selected.category?.icon || selected.category?.name || ""}
                    size={12}
                  />
                }
                color={selected.category?.color}
              >
                {selected.category?.name || "Session"}
              </Tag>
              <div className="min-w-0 flex-1">
                <strong className="text-sm">{selected.title}</strong>
                <div className="text-xs text-muted">
                  {formatSessionWhen(selected)} · {selected.status}
                </div>
              </div>
              <IconButton label="Close" onClick={() => setSelected(null)}>
                <X size={16} />
              </IconButton>
            </div>
            <div className="mt-2 flex flex-wrap gap-2">
              {selected.status === "proposed" && selected.proposal_batch_id && (
                <Button tone="primary" disabled={busy} onClick={onApproveSelected}>
                  <CheckCircle size={16} /> Approve batch
                </Button>
              )}
              {selected.status === "scheduled" && (
                <>
                  <Button tone="primary" disabled={busy} onClick={() => onOutcome("completed")}>
                    <CheckCircle size={16} /> Completed
                  </Button>
                  <Button tone="danger" disabled={busy} onClick={() => onOutcome("missed")}>
                    Missed
                  </Button>
                </>
              )}
              <Button tone="danger" disabled={busy} onClick={() => setPendingDelete(selected)}>
                <Trash size={16} /> Delete
              </Button>
            </div>
          </FrostFloat>
        )}
      </div>

      {sidebarOpen && (
        <aside
          className="fixed inset-y-0 right-0 z-20 flex h-full w-[22rem] flex-col gap-3 overflow-y-auto bg-page p-3 xl:static xl:z-0 xl:w-80 xl:shrink-0 xl:bg-transparent xl:p-0"
          aria-label="Calendar panel"
        >
          <div className="mb-1 flex items-center justify-between xl:hidden">
            <h2 className="m-0 text-sm font-medium">Calendar</h2>
            <IconButton label="Close panel" onClick={() => setSidebarOpen(false)}>
              <X size={16} />
            </IconButton>
          </div>

          <Surface className="rounded-3xl border-0 bg-raised/90">
            <div className="mb-3 flex items-center justify-between">
              <h3 className="m-0 text-sm font-medium capitalize">{miniLabel}</h3>
              <div className="flex gap-1">
                <IconButton
                  label="Previous month"
                  size="sm"
                  onClick={() =>
                    setMiniCursor((c) => {
                      const d = new Date(c.year, c.month - 1, 1);
                      return { year: d.getFullYear(), month: d.getMonth() };
                    })
                  }
                >
                  <CaretLeft size={14} />
                </IconButton>
                <IconButton
                  label="Next month"
                  size="sm"
                  onClick={() =>
                    setMiniCursor((c) => {
                      const d = new Date(c.year, c.month + 1, 1);
                      return { year: d.getFullYear(), month: d.getMonth() };
                    })
                  }
                >
                  <CaretRight size={14} />
                </IconButton>
              </div>
            </div>
            <div className="mb-1 grid grid-cols-7 text-center text-[10px] text-muted">
              {WEEKDAY_LABELS.map((d) => (
                <span key={d} className="py-1">
                  {d.slice(0, 2)}
                </span>
              ))}
            </div>
            <div className="grid grid-cols-7 gap-y-1 text-center text-sm">
              {(() => {
                const weekStart = new Date(selectedDay);
                const offset = (weekStart.getDay() + 6) % 7;
                weekStart.setDate(weekStart.getDate() - offset);
                weekStart.setHours(0, 0, 0, 0);
                const weekEnd = new Date(weekStart);
                weekEnd.setDate(weekEnd.getDate() + 6);
                return miniCells.map((cell, i) => {
                  const selected = sameDay(cell.date, selectedDay);
                  const t = cell.date.getTime();
                  const inWeek = t >= weekStart.getTime() && t <= weekEnd.getTime();
                  const col = i % 7;
                  const isWeekStart = inWeek && col === 0;
                  const isWeekEnd = inWeek && col === 6;
                  return (
                    <button
                      key={`${cell.date.toISOString()}-${i}`}
                      type="button"
                      onClick={() => jumpToDay(cell.date)}
                      className={cn(
                        "relative flex h-8 items-center justify-center text-sm transition-colors",
                        !cell.inMonth && "text-muted-dim",
                        inWeek && "bg-raised-soft",
                        isWeekStart && "rounded-l-full",
                        isWeekEnd && "rounded-r-full",
                        cell.inMonth && !selected && "text-ink hover:text-mint",
                      )}
                    >
                      <span
                        className={cn(
                          "flex size-7 items-center justify-center rounded-full",
                          selected && "bg-mint font-medium text-page-deep",
                        )}
                      >
                        {cell.day}
                      </span>
                    </button>
                  );
                });
              })()}
            </div>
          </Surface>

          <Surface className="rounded-3xl border-0 bg-raised/90">
            {nextUp ? (
              <>
                <div className="mb-2 flex items-start justify-between gap-2">
                  <span className="text-xs text-muted">{formatRange(nextUp)}</span>
                  <span className="inline-flex items-center gap-1 rounded-pill bg-mint/15 px-2 py-0.5 text-[10px] text-mint">
                    <Clock size={11} />
                    {minutesUntil(nextUp)}
                  </span>
                </div>
                <strong className="block text-sm leading-snug">{nextUp.title}</strong>
                <div className="mt-3 flex gap-2">
                  <Button tone="ghost" onClick={() => setSelected(null)}>
                    Later
                  </Button>
                  <Button tone="primary" onClick={() => setSelected(nextUp)}>
                    Details
                  </Button>
                </div>
              </>
            ) : (
              <p className="m-0 text-sm text-muted">Nothing upcoming.</p>
            )}
          </Surface>

          <Surface className="rounded-3xl border-0 bg-raised/90">
            <div className="mb-3 flex items-center justify-between">
              <h3 className="m-0 text-sm font-medium">Categories</h3>
              <IconButton label="Add category" size="sm" onClick={() => setAddCategoryOpen(true)}>
                <Plus size={14} weight="bold" />
              </IconButton>
            </div>
            <ul className="m-0 flex list-none flex-col gap-3 p-0">
              {uniqueCategories.map((cat) => {
                const count = counts[cat.id] || 0;
                const pct = Math.min(100, Math.round((count / categoryTotals) * 100));
                return (
                  <li key={cat.id} className="flex items-center gap-2">
                    <label className="flex min-w-0 flex-1 cursor-pointer items-center gap-2">
                      <input
                        type="checkbox"
                        className="sr-only"
                        checked={enabled[cat.id] !== false}
                        onChange={(e) =>
                          setEnabled((prev) => ({ ...prev, [cat.id]: e.target.checked }))
                        }
                      />
                      <span
                        className={cn(
                          "size-2.5 shrink-0 rounded-full",
                          enabled[cat.id] === false && "opacity-30",
                        )}
                        style={{ background: cat.color }}
                      />
                      <span
                        className={cn(
                          "w-20 shrink-0 truncate text-sm",
                          enabled[cat.id] === false && "text-muted",
                        )}
                      >
                        {cat.name}
                      </span>
                      <span className="h-1.5 min-w-0 flex-1 overflow-hidden rounded-full bg-raised-soft">
                        <span
                          className="block h-full rounded-full transition-[width]"
                          style={{
                            width: `${enabled[cat.id] === false ? 0 : Math.max(pct, count ? 12 : 0)}%`,
                            background: cat.color,
                          }}
                        />
                      </span>
                    </label>
                    {cat.name !== "Other" && cat.name !== "Fixed" && (
                      <IconButton
                        label="Delete category"
                        size="sm"
                        onClick={() => onDeleteCategory(cat.id)}
                      >
                        <Trash size={14} />
                      </IconButton>
                    )}
                  </li>
                );
              })}
            </ul>
            {addCategoryOpen && (
              <div className="mt-3 flex flex-col gap-2 border-t border-white/5 pt-3">
                <input
                  value={newName}
                  onChange={(e) => setNewName(e.target.value)}
                  placeholder="Name"
                  autoFocus
                  className="rounded-lg bg-raised-soft px-2 py-1 text-sm outline-none"
                />
                <input
                  value={newKeywords}
                  onChange={(e) => setNewKeywords(e.target.value)}
                  placeholder="Keywords"
                  className="rounded-lg bg-raised-soft px-2 py-1 text-sm outline-none"
                />
                <div className="flex flex-wrap gap-1.5">
                  {COLOR_PRESETS.map((c) => (
                    <button
                      key={c}
                      type="button"
                      className={cn(
                        "size-5 rounded-full",
                        newColor === c && "ring-2 ring-mint ring-offset-1 ring-offset-raised",
                      )}
                      style={{ background: c }}
                      onClick={() => setNewColor(c)}
                      aria-label={c}
                    />
                  ))}
                </div>
                <div className="flex gap-2">
                  <Button tone="ghost" onClick={closeAddCategory}>
                    Cancel
                  </Button>
                  <Button
                    tone="primary"
                    disabled={busy || !newName.trim()}
                    onClick={onAddCategory}
                  >
                    Add
                  </Button>
                </div>
              </div>
            )}
          </Surface>

          <Surface className="rounded-3xl border-0 bg-raised/90">
            <div className="mb-1 flex items-center justify-between">
              <h3 className="m-0 text-sm font-medium">Busy hours</h3>
              <IconButton label="Add busy hours" size="sm" onClick={() => openFixedEditor()}>
                <Plus size={14} weight="bold" />
              </IconButton>
            </div>
            <p className="mt-0 mb-3 text-xs text-muted">Skipped when proposing.</p>
            <ul className="m-0 flex list-none flex-col gap-3 p-0">
              {busyGroups.map((g) => (
                <li key={g.ids.join("-")} className="flex items-start justify-between gap-2">
                  <div className="min-w-0">
                    <strong className="text-sm">{g.title}</strong>
                    <div className="text-xs text-muted">
                      {weekdayRangeLabel(g.weekdays)} ·{" "}
                      {minutesToTime(g.start_minute).slice(0, 5)}–
                      {minutesToTime(g.end_minute).slice(0, 5)}
                    </div>
                  </div>
                  <div className="flex shrink-0">
                    <Button
                      tone="ghost"
                      disabled={busy}
                      onClick={() => {
                        const block = fixedBlocks.find((b) => b.id === g.ids[0]);
                        if (block) openFixedEditor(block);
                      }}
                    >
                      Edit
                    </Button>
                    <IconButton
                      label="Delete"
                      disabled={busy}
                      onClick={() => onDeleteFixedGroup(g.ids)}
                    >
                      <Trash size={14} />
                    </IconButton>
                  </div>
                </li>
              ))}
              {!busyGroups.length && (
                <li className="text-xs text-muted">No busy hours yet.</li>
              )}
            </ul>
          </Surface>
        </aside>
      )}

      {eventOpen && (
        <div
          className="fixed inset-0 z-30 grid place-items-center bg-overlay p-4"
          onClick={() => setEventOpen(false)}
        >
          <FrostFloat
            className="w-full max-w-md p-5"
            role="dialog"
            aria-label="Add event"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="mb-3 flex items-center justify-between">
              <h2 className="m-0 font-display text-lg">Add event</h2>
              <IconButton label="Close" onClick={() => setEventOpen(false)}>
                <X size={16} />
              </IconButton>
            </div>
            <div className="flex flex-col gap-2 text-sm">
              <label className="flex flex-col gap-1">
                Title
                <input
                  value={eventTitle}
                  onChange={(e) => setEventTitle(e.target.value)}
                  placeholder="Title"
                  autoFocus
                  className="rounded-lg bg-raised-soft px-2 py-1.5 outline-none"
                />
              </label>
              <label className="flex flex-col gap-1">
                Date
                <input
                  type="date"
                  value={eventDate}
                  onChange={(e) => setEventDate(e.target.value)}
                  className="rounded-lg bg-raised-soft px-2 py-1.5 outline-none"
                />
              </label>
              <div className="grid grid-cols-2 gap-2">
                <label className="flex flex-col gap-1">
                  Start
                  <input
                    type="time"
                    value={eventStart}
                    onChange={(e) => setEventStart(e.target.value)}
                    className="rounded-lg bg-raised-soft px-2 py-1.5 outline-none"
                  />
                </label>
                <label className="flex flex-col gap-1">
                  End
                  <input
                    type="time"
                    value={eventEnd}
                    onChange={(e) => setEventEnd(e.target.value)}
                    className="rounded-lg bg-raised-soft px-2 py-1.5 outline-none"
                  />
                </label>
              </div>
              <label className="flex flex-col gap-1">
                Category
                <select
                  value={eventCategoryId}
                  onChange={(e) => setEventCategoryId(e.target.value)}
                  className="rounded-lg bg-raised-soft px-2 py-1.5 outline-none"
                >
                  <option value="">Auto</option>
                  {uniqueCategories.map((cat) => (
                    <option key={cat.id} value={cat.id}>
                      {cat.name}
                    </option>
                  ))}
                </select>
              </label>
              <Button
                tone="primary"
                block
                disabled={busy || !eventTitle.trim()}
                onClick={onAddEvent}
              >
                <Plus size={16} weight="bold" /> Add event
              </Button>
            </div>
          </FrostFloat>
        </div>
      )}

      {fixedOpen && (
        <div
          className="fixed inset-0 z-30 grid place-items-center bg-overlay p-4"
          onClick={() => setFixedOpen(false)}
        >
          <FrostFloat
            className="w-full max-w-md p-5"
            role="dialog"
            aria-label="Fixed block"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="mb-3 flex items-center justify-between">
              <h2 className="m-0 font-display text-lg">
                {editingFixedId ? "Edit busy hours" : "Add busy hours"}
              </h2>
              <IconButton label="Close" onClick={() => setFixedOpen(false)}>
                <X size={16} />
              </IconButton>
            </div>
            <div className="flex flex-col gap-2 text-sm">
              <label className="flex flex-col gap-1">
                Title
                <input
                  value={fixedTitle}
                  onChange={(e) => setFixedTitle(e.target.value)}
                  className="rounded-lg bg-raised-soft px-2 py-1.5 outline-none"
                />
              </label>
              <label className="flex flex-col gap-1">
                Weekday
                <select
                  value={fixedWeekday}
                  onChange={(e) => setFixedWeekday(Number(e.target.value))}
                  className="rounded-lg bg-raised-soft px-2 py-1.5 outline-none"
                >
                  {WEEKDAY_LABELS.map((label, i) => (
                    <option key={label} value={i}>
                      {label}
                    </option>
                  ))}
                </select>
              </label>
              <div className="grid grid-cols-2 gap-2">
                <label className="flex flex-col gap-1">
                  Start
                  <input
                    type="time"
                    value={fixedStart}
                    onChange={(e) => setFixedStart(e.target.value)}
                    className="rounded-lg bg-raised-soft px-2 py-1.5 outline-none"
                  />
                </label>
                <label className="flex flex-col gap-1">
                  End
                  <input
                    type="time"
                    value={fixedEnd}
                    onChange={(e) => setFixedEnd(e.target.value)}
                    className="rounded-lg bg-raised-soft px-2 py-1.5 outline-none"
                  />
                </label>
              </div>
              <Button tone="primary" block disabled={busy || !fixedTitle.trim()} onClick={onSaveFixed}>
                Save
              </Button>
            </div>
          </FrostFloat>
        </div>
      )}

      <ConfirmDialog
        open={!!pendingDelete}
        title="Delete event?"
        message={
          pendingDelete ? (
            <>
              Delete <strong>{pendingDelete.title}</strong>? This cannot be undone.
            </>
          ) : null
        }
        confirmLabel="Delete"
        cancelLabel="Cancel"
        danger
        busy={busy}
        onConfirm={confirmDeleteSession}
        onCancel={() => !busy && setPendingDelete(null)}
      />
    </section>
  );
}
