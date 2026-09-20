import { useEffect, useMemo, useRef, useState } from "react";
import FullCalendar from "@fullcalendar/react";
import dayGridPlugin from "@fullcalendar/daygrid";
import timeGridPlugin from "@fullcalendar/timegrid";
import interactionPlugin from "@fullcalendar/interaction";
import type { DayHeaderContentArg, EventContentArg } from "@fullcalendar/core";
import {
  CaretLeft,
  CheckCircle,
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

  async function onDeleteFixed(id: string) {
    if (busy) return;
    setBusy(true);
    try {
      await deleteFixedBlock(id);
      await reload();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Could not delete fixed block");
    } finally {
      setBusy(false);
    }
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
    <section className="flex h-full min-h-0 bg-page">
      {sidebarOpen && (
        <aside
          className="fixed inset-y-0 left-0 z-20 h-full w-64 overflow-y-auto bg-sidebar p-3 lg:static lg:z-0"
          aria-label="Categories"
        >
          <div className="mb-3 flex items-center justify-between">
            <h2 className="m-0 text-sm font-medium">Categories</h2>
            <IconButton label="Close categories" onClick={() => setSidebarOpen(false)}>
              <CaretLeft size={16} />
            </IconButton>
          </div>

          <ul className="m-0 flex list-none flex-col gap-1 p-0">
            {categories.map((cat) => (
              <li key={cat.id} className="flex items-center gap-1">
                <label className="flex min-w-0 flex-1 cursor-pointer items-center gap-2 rounded-xl px-1 py-1 hover:bg-sidebar-hover">
                  <input
                    type="checkbox"
                    checked={enabled[cat.id] !== false}
                    onChange={(e) =>
                      setEnabled((prev) => ({ ...prev, [cat.id]: e.target.checked }))
                    }
                  />
                  <Tag icon={<CategoryIcon name={cat.icon || cat.name} size={12} />} color={cat.color}>
                    {cat.name}
                  </Tag>
                  <span className="ml-auto text-[10px] text-muted-dim">{counts[cat.id] || 0}</span>
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
            ))}
          </ul>

          <div className="mt-3">
            {!addCategoryOpen ? (
              <Button tone="ghost" block onClick={() => setAddCategoryOpen(true)}>
                <Plus size={16} weight="bold" /> Add
              </Button>
            ) : (
              <div className="flex flex-col gap-2">
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
                        newColor === c && "ring-2 ring-mint ring-offset-1 ring-offset-sidebar",
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
          </div>

          <div className="mt-5">
            <div className="mb-1 flex items-center justify-between">
              <h2 className="m-0 text-sm font-medium">Busy hours</h2>
              <IconButton label="Add fixed block" onClick={() => openFixedEditor()}>
                <Plus size={16} weight="bold" />
              </IconButton>
            </div>
            <p className="mt-0 mb-2 text-xs text-muted">Skipped when proposing.</p>
            <ul className="m-0 flex list-none flex-col gap-2 p-0">
              {fixedBlocks.map((b) => (
                <li key={b.id} className="flex items-start justify-between gap-2">
                  <div>
                    <strong className="text-sm">{b.title}</strong>
                    <div className="text-xs text-muted">
                      {WEEKDAY_LABELS[b.weekday]} ·{" "}
                      {minutesToTime(b.start_minute).slice(0, 5)}–
                      {minutesToTime(b.end_minute).slice(0, 5)}
                    </div>
                  </div>
                  <div className="flex">
                    <Button tone="ghost" disabled={busy} onClick={() => openFixedEditor(b)}>
                      Edit
                    </Button>
                    <IconButton
                      label="Delete"
                      disabled={busy}
                      onClick={() => onDeleteFixed(b.id)}
                    >
                      <Trash size={14} />
                    </IconButton>
                  </div>
                </li>
              ))}
            </ul>
          </div>
        </aside>
      )}

      <div className="flex min-h-0 min-w-0 flex-1 flex-col p-4">
        <header className="mb-3 flex flex-wrap items-center gap-2">
          <Button
            tone="ghost"
            className={sidebarOpen ? "lg:hidden" : ""}
            onClick={() => setSidebarOpen((v) => !v)}
            aria-label={sidebarOpen ? "Close categories" : "Open categories"}
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
                    {categories.map((cat) => (
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
