import { useEffect } from "react";
import { WarningCircle } from "@phosphor-icons/react";
import type { CreateEventInput } from "@buddy/calendar/models";
import { addDays, addMonths, startOfWeek } from "@buddy/calendar/utils";
import { useCalendarStore } from "../../stores/useCalendarStore";
import { useCalendarNotificationStore } from "../../stores/useCalendarNotificationStore";
import {
  useLifestyleStore,
  visibleScheduleBlocks,
} from "../../stores/useLifestyleStore";
import { AgendaView } from "./AgendaView";
import { CalendarHeader } from "./CalendarHeader";
import { CalendarSidebar } from "./CalendarSidebar";
import { DeleteConfirmDialog } from "./DeleteConfirmDialog";
import { DreamLogPanel } from "./DreamLogPanel";
import { EventDetailsDrawer } from "./EventDetailsDrawer";
import { EventFormModal } from "./EventFormModal";
import { MonthView } from "./MonthView";
import { NotificationPanel } from "./NotificationPanel";
import { TimeGridView } from "./TimeGridView";
import { WorkDashboardPanel } from "./WorkDashboardPanel";

export function CalendarWorkspace() {
  const {
    events,
    view,
    cursorDate,
    loading,
    error,
    selectedEventId,
    formOpen,
    formMode,
    draftDefaults,
    searchQuery,
    enabledCategories,
    deleteConfirmId,
    setView,
    setCursorDate,
    selectEvent,
    setFormOpen,
    setSearchQuery,
    toggleCategory,
    setDeleteConfirmId,
    clearError,
    loadRange,
    createEvent,
    updateEvent,
    deleteEvent,
    duplicateEvent,
  } = useCalendarStore();

  const {
    scheduleBlocks,
    showWork,
    showSleep,
    selectedBlockId,
    dreams,
    workStats,
    workDayLog,
    panelLoading,
    setShowWork,
    setShowSleep,
    selectBlock,
    loadBlocks,
    addDream,
    removeDream,
    saveSales,
    saveEndTime,
  } = useLifestyleStore();

  const { count, panelOpen, setPanelOpen } = useCalendarNotificationStore();

  useEffect(() => {
    void loadRange();
    void loadBlocks();
  }, [loadRange, loadBlocks, view, cursorDate]);

  const selected = events.find((e) => e.id === selectedEventId) ?? null;
  const selectedBlock =
    scheduleBlocks.find((b) => b.id === selectedBlockId) ?? null;
  const visibleBlocks = visibleScheduleBlocks(
    scheduleBlocks,
    showWork,
    showSleep,
  );

  function openCreate(day?: Date, hour?: number) {
    const base = day ? new Date(day) : new Date(cursorDate);
    if (hour !== undefined) base.setHours(hour, 0, 0, 0);
    else if (!day) {
      /* keep cursor */
    } else {
      base.setHours(9, 0, 0, 0);
    }
    const draft: CreateEventInput = {
      title: "",
      start_time: base.getTime(),
      end_time: base.getTime() + 60 * 60 * 1000,
      all_day: false,
      category: "general",
      timezone: Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC",
      reminders: [{ minutes_before: 15, method: "popup" }],
    };
    selectBlock(null);
    setFormOpen(true, "create", draft);
  }

  function navigate(dir: -1 | 1) {
    if (view === "month") setCursorDate(addMonths(cursorDate, dir));
    else if (view === "week") setCursorDate(addDays(cursorDate, dir * 7));
    else setCursorDate(addDays(cursorDate, dir));
  }

  const weekDays = Array.from({ length: 7 }, (_, i) =>
    addDays(startOfWeek(cursorDate), i),
  );

  return (
    <div className="relative flex min-h-0 flex-1 gap-0 overflow-hidden p-4">
      <CalendarSidebar
        cursorDate={cursorDate}
        searchQuery={searchQuery}
        enabledCategories={enabledCategories}
        showWork={showWork}
        showSleep={showSleep}
        events={events}
        onSearch={setSearchQuery}
        onToggleCategory={toggleCategory}
        onToggleWork={() => setShowWork(!showWork)}
        onToggleSleep={() => setShowSleep(!showSleep)}
        onSelectDay={(d) => {
          setCursorDate(d);
          if (view === "month") setView("day");
        }}
        onCreate={() => openCreate()}
        onSelectEvent={(id) => {
          selectBlock(null);
          selectEvent(id);
        }}
      />

      <div className="relative flex min-h-0 min-w-0 flex-1 flex-col pl-4">
        <CalendarHeader
          cursorDate={cursorDate}
          view={view}
          notificationCount={count}
          onPrev={() => navigate(-1)}
          onNext={() => navigate(1)}
          onToday={() => setCursorDate(new Date())}
          onViewChange={setView}
          onToggleNotifications={() => setPanelOpen(!panelOpen)}
        />
        <NotificationPanel />

        {error && (
          <div className="mb-3 flex items-center gap-2 rounded-xl border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-xs text-amber-200">
            <WarningCircle size={16} />
            <span className="flex-1">{error}</span>
            <button
              type="button"
              onClick={clearError}
              className="text-amber-300/80 hover:text-amber-100"
            >
              Dismiss
            </button>
          </div>
        )}

        {loading && events.length === 0 ? (
          <div className="flex min-h-0 flex-1 items-center justify-center rounded-2xl border border-zinc-800 text-sm text-zinc-500">
            Loading calendar…
          </div>
        ) : view === "month" ? (
          <MonthView
            cursorDate={cursorDate}
            events={events}
            selectedEventId={selectedEventId}
            onSelectDay={(d) => setCursorDate(d)}
            onSelectEvent={(id) => {
              selectBlock(null);
              selectEvent(id);
            }}
            onCreateAt={(d) => openCreate(d)}
          />
        ) : view === "agenda" ? (
          <AgendaView
            events={events}
            selectedEventId={selectedEventId}
            onSelectEvent={(id) => {
              selectBlock(null);
              selectEvent(id);
            }}
          />
        ) : (
          <TimeGridView
            days={view === "day" ? [cursorDate] : weekDays}
            events={events}
            scheduleBlocks={visibleBlocks}
            selectedEventId={selectedEventId}
            selectedBlockId={selectedBlockId}
            onSelectEvent={(id) => {
              selectBlock(null);
              selectEvent(id);
            }}
            onSelectBlock={selectBlock}
            onCreateAt={(day, hour) => openCreate(day, hour)}
          />
        )}
      </div>

      {!selectedBlock && (
        <EventDetailsDrawer
          event={selected}
          onClose={() => selectEvent(null)}
          onEdit={() => {
            if (!selected) return;
            setFormOpen(true, "edit", {
              title: selected.title,
              description: selected.description,
              location: selected.location,
              category: selected.category,
              color: selected.color,
              start_time: selected.start_time,
              end_time: selected.end_time,
              all_day: selected.all_day,
              timezone: selected.timezone,
              reminders: selected.reminders,
              recurrence: selected.recurrence,
            });
          }}
          onDuplicate={() => {
            if (selected) void duplicateEvent(selected.id);
          }}
          onDelete={() => {
            if (selected) setDeleteConfirmId(selected.id);
          }}
        />
      )}

      {selectedBlock?.kind === "sleep" && (
        <DreamLogPanel
          block={selectedBlock}
          dreams={dreams}
          loading={panelLoading}
          onClose={() => selectBlock(null)}
          onAdd={async (body) => {
            await addDream({ body });
          }}
          onDelete={async (id) => {
            await removeDream(id);
          }}
        />
      )}

      {selectedBlock?.kind === "work" && (
        <WorkDashboardPanel
          block={selectedBlock}
          stats={workStats}
          dayLog={workDayLog}
          loading={panelLoading}
          onClose={() => selectBlock(null)}
          onSaveSales={saveSales}
          onSaveEndTime={saveEndTime}
        />
      )}

      {formOpen && draftDefaults && (
        <EventFormModal
          mode={formMode}
          initial={{
            title: draftDefaults.title,
            description: draftDefaults.description,
            location: draftDefaults.location,
            category: draftDefaults.category ?? "general",
            color: draftDefaults.color,
            start_time: draftDefaults.start_time,
            end_time: draftDefaults.end_time,
            all_day: draftDefaults.all_day,
            timezone:
              draftDefaults.timezone ||
              Intl.DateTimeFormat().resolvedOptions().timeZone ||
              "UTC",
            reminders: draftDefaults.reminders,
            recurrence: draftDefaults.recurrence,
          }}
          onClose={() => setFormOpen(false)}
          onSubmit={async (input) => {
            if (formMode === "edit" && selectedEventId) {
              await updateEvent(selectedEventId, {
                title: input.title,
                description: input.description ?? "",
                location: input.location ?? "",
                category: input.category,
                color: input.color ?? "",
                start_time: input.start_time,
                end_time: input.end_time,
                all_day: input.all_day,
                timezone: input.timezone,
                recurrence: input.recurrence ?? undefined,
                clear_recurrence: !input.recurrence,
                reminders: input.reminders ?? [],
              });
            } else {
              await createEvent(input);
            }
          }}
        />
      )}

      {deleteConfirmId && (
        <DeleteConfirmDialog
          title={
            events.find((e) => e.id === deleteConfirmId)?.title ?? "Event"
          }
          onCancel={() => setDeleteConfirmId(null)}
          onConfirm={() => void deleteEvent(deleteConfirmId)}
        />
      )}
    </div>
  );
}
