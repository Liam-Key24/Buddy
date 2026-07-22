export type ScheduleKind = "work" | "sleep";

export interface ScheduleBlock {
  id: string;
  kind: ScheduleKind;
  title: string;
  start_time: number;
  end_time: number;
  anchor_date: string;
}

export interface DreamEntry {
  id: string;
  sleep_date: string;
  title?: string | null;
  body: string;
  tags: string[];
  mood?: number | null;
  sleep_quality?: number | null;
  created_at: number;
  updated_at: number;
}

export interface CreateDreamInput {
  body: string;
  sleep_date?: string | null;
  title?: string | null;
  tags?: string[];
  mood?: number | null;
  sleep_quality?: number | null;
}

export interface UpdateDreamInput {
  body?: string | null;
  title?: string | null;
  tags?: string[] | null;
  mood?: number | null;
  sleep_quality?: number | null;
}

export interface WorkDayLog {
  work_date: string;
  actual_start_ms?: number | null;
  actual_end_ms?: number | null;
  sales_amount: number;
  sales_currency: string;
  notes?: string | null;
  updated_at: number;
}

export interface WorkPeriodStats {
  hours: number;
  sales: number;
  currency: string;
}

export interface WorkStats {
  today: WorkPeriodStats;
  week: WorkPeriodStats;
  month: WorkPeriodStats;
}

export const SCHEDULE_LAYER = {
  work: { id: "work", label: "Work schedule", color: "#F97316" },
  sleep: { id: "sleep", label: "Sleep schedule", color: "#6366F1" },
} as const;
