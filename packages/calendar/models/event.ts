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
