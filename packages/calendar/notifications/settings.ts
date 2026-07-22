export interface CalendarNotificationSettings {
  desktopEnabled: boolean;
  defaultTimezone: string;
  defaultRemindersJson: string;
}

export const DEFAULT_NOTIFICATION_SETTINGS: CalendarNotificationSettings = {
  desktopEnabled: true,
  defaultTimezone: Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC",
  defaultRemindersJson: JSON.stringify([
    { minutes_before: 15, method: "popup" },
  ]),
};
