export interface ReminderDelivery {
  id: string;
  event_id: string;
  event_title: string;
  reminder_minutes: number;
  fire_at: number;
  status: string;
  snoozed_until?: number | null;
  delivered_at?: number | null;
}
