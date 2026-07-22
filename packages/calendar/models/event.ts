export interface RecurrenceRule {
  frequency: string;
  interval: number;
  until?: number | null;
  count?: number | null;
  by_day?: string[];
}

export interface Reminder {
  minutes_before: number;
  method: string;
}

export type Flexibility = "fixed" | "flexible" | "optional";
export type EventPriority = "low" | "normal" | "high";

export interface CalendarEvent {
  id: string;
  title: string;
  description?: string | null;
  location?: string | null;
  category: string;
  color?: string | null;
  start_time: number;
  end_time: number;
  all_day: boolean;
  timezone: string;
  recurrence?: RecurrenceRule | null;
  reminders: Reminder[];
  external_provider?: string | null;
  external_event_id?: string | null;
  sync_status: string;
  created_at: number;
  updated_at: number;
  occurrence_of?: string | null;
  flexibility?: Flexibility;
  priority?: EventPriority;
}

export interface CreateEventInput {
  title: string;
  description?: string | null;
  location?: string | null;
  category?: string | null;
  color?: string | null;
  start_time: number;
  end_time: number;
  all_day?: boolean;
  timezone?: string | null;
  recurrence?: RecurrenceRule | null;
  reminders?: Reminder[];
  flexibility?: Flexibility | null;
  priority?: EventPriority | null;
  /** Skip conflict checks (UI explicit creates default to true). */
  force?: boolean;
}

export interface UpdateEventInput {
  title?: string | null;
  description?: string | null;
  location?: string | null;
  category?: string | null;
  color?: string | null;
  start_time?: number | null;
  end_time?: number | null;
  all_day?: boolean | null;
  timezone?: string | null;
  recurrence?: RecurrenceRule | null;
  clear_recurrence?: boolean;
  reminders?: Reminder[] | null;
  flexibility?: Flexibility | null;
  priority?: EventPriority | null;
  force?: boolean;
}

export interface DayCapacity {
  date: string;
  booked_hours: number;
  meeting_hours: number;
  focus_hours: number;
  free_hours: number;
  waking_hours: number;
  overloaded: boolean;
}

export interface DaySummarySuggestion {
  action: string;
  message: string;
  event_id?: string | null;
  start?: number | null;
  end?: number | null;
}

export interface DaySummary {
  date: string;
  capacity: DayCapacity;
  meetings: { title: string; start: number; end: number }[];
  focus_blocks: { title: string; start: number; end: number }[];
  free_slots: { title: string; start: number; end: number }[];
  conflicts: string[];
  suggestions: DaySummarySuggestion[];
}

export type CalendarView = "month" | "week" | "day" | "agenda";

export interface CategoryDef {
  id: string;
  label: string;
  color: string;
}

export const CATEGORIES: CategoryDef[] = [
  { id: "work", label: "Work", color: "#3B82F6" },
  { id: "personal", label: "Personal", color: "#8B5CF6" },
  { id: "birthdays", label: "Birthdays", color: "#10B981" },
  { id: "holidays", label: "Holidays", color: "#F59E0B" },
  { id: "general", label: "General", color: "#64748B" },
];

export const FLEXIBILITY_OPTIONS: { id: Flexibility; label: string }[] = [
  { id: "fixed", label: "Fixed" },
  { id: "flexible", label: "Flexible" },
  { id: "optional", label: "Optional" },
];
