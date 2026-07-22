import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import { useChatStore } from "../stores/useChatStore";
import { useCodeAgentStore } from "../stores/useCodeAgentStore";
import { useConversationStore } from "../stores/useConversationStore";
import type {
  CalendarEvent,
  CreateEventInput,
  ReminderDelivery,
  UpdateEventInput,
  CreateDreamInput,
  DreamEntry,
  ScheduleBlock,
  UpdateDreamInput,
  WorkDayLog,
  WorkStats,
} from "@buddy/calendar/models";

export interface ServiceStatusResponse {
  mlx: boolean;
  brain: boolean;
}

export async function fetchServiceStatus(): Promise<ServiceStatusResponse> {
  return invoke("get_service_status");
}

export async function startBrain(): Promise<void> {
  return invoke("start_brain");
}

interface ConversationDto {
  id: string;
  title: string;
  created_at: number;
  updated_at: number;
  kind?: string;
  focus_mode?: string | null;
  workspace_path?: string | null;
}

export async function loadConversations() {
  const conversations =
    await invoke<Array<ConversationDto>>("list_conversations");
  useConversationStore.getState().setConversations(conversations);
  return conversations;
}

export async function createConversation(title?: string) {
  const conv = await invoke<{
    id: string;
    title: string;
    created_at: number;
    updated_at: number;
  }>("create_conversation", { title });
  useConversationStore.getState().addConversation(conv);
  return conv;
}

export async function deleteConversation(id: string) {
  try {
    await invoke("delete_conversation", { id });
  } finally {
    await loadConversations();
  }
}

export async function loadMessages(
  conversationId: string,
  options?: { force?: boolean },
) {
  if (!options?.force && useChatStore.getState().isStreaming) {
    return;
  }

  const messages = await invoke<
    Array<{
      id: string;
      conversation_id: string;
      role: string;
      content: string;
      created_at: number;
    }>
  >("get_messages", { conversationId });
  useChatStore.getState().setMessages(
    messages.map((m) => ({
      id: m.id,
      role: m.role as "user" | "assistant",
      content: m.content,
      created_at: m.created_at,
    })),
  );
}

export async function sendMessage(
  conversationId: string,
  text: string,
  options?: { skipOptimistic?: boolean },
) {
  const chat = useChatStore.getState();
  if (!options?.skipOptimistic) {
    chat.beginSend(text);
  }

  const unlistenChunk = await listen<string>("chat-chunk", (event) => {
    useChatStore.getState().appendStreaming(event.payload);
  });
  const unlistenDone = await listen("chat-done", async () => {
    await loadMessages(conversationId, { force: true });
    useChatStore.getState().clearStreaming();
    unlistenChunk();
    unlistenDone();
  });

  try {
    await invoke("send_message", { conversationId, text });
    await loadConversations();
  } catch (error) {
    chat.clearStreaming();
    unlistenChunk();
    unlistenDone();
    throw error;
  }
}

export interface FullSettings {
  mlx_url: string;
  brain_url: string;
  model_name: string;
  log_level: string;
  auto_start_mlx: boolean;
  model_name_chat: string;
  model_name_code: string;
  llm_profile_router: string;
  codex_model: string;
  codex_workspace: string;
  code_agent_backend: string;
  code_model: string;
  cursor_path: string;
  codex_path: string;
  email_signature: string;
  email_greeting: string;
  email_body_template: string;
  fs_excluded_paths: string[];
  calendar_notifications_enabled: boolean;
  calendar_default_timezone: string;
  calendar_default_reminders_json: string;
}

export async function loadSettings() {
  return invoke<FullSettings>("get_settings");
}

export async function saveSetting(key: string, value: string) {
  await invoke("set_setting", { key, value });
}

export async function setExcludedPaths(paths: string[]) {
  await invoke("set_setting", {
    key: "fs_excluded_paths",
    value: JSON.stringify(paths),
  });
}

export async function setSecret(key: string, value: string) {
  await invoke("set_secret", { key, value });
}

export async function deleteSecret(key: string) {
  await invoke("delete_secret", { key });
}

export async function getSecretStatus() {
  return invoke<Record<string, boolean>>("get_secret_status");
}

export interface ExternalAction {
  id: string;
  action_type: string;
  summary: string;
  detail_json?: string | null;
  approved: boolean;
  created_at: number;
}

export async function listExternalActions(limit = 100) {
  return invoke<ExternalAction[]>("list_external_actions", { limit });
}

export interface RefreshCacheResult {
  memories_reindexed: number;
  duration_ms: number;
}

export async function refreshCache() {
  return invoke<RefreshCacheResult>("refresh_cache");
}

export async function listCodexConversations() {
  return invoke<ConversationDto[]>("list_codex_conversations");
}

export async function createCodexConversation(
  title?: string,
  focusMode?: string,
  workspacePath?: string,
) {
  return invoke<ConversationDto>("create_codex_conversation", {
    title: title ?? null,
    focusMode: focusMode ?? null,
    workspacePath: workspacePath ?? null,
  });
}

export async function setConversationFocus(id: string, focusMode: string) {
  await invoke("set_conversation_focus", { id, focusMode });
}

export async function sendCodexMessage(
  conversationId: string,
  text: string,
  focus: string,
  attachments: string[],
) {
  await invoke("send_codex_message", {
    conversationId,
    text,
    focus,
    attachments,
  });
}

export async function loadCodexMessages(conversationId: string) {
  const messages = await invoke<
    Array<{
      id: string;
      conversation_id: string;
      role: string;
      content: string;
      created_at: number;
    }>
  >("get_messages", { conversationId });
  useCodeAgentStore.getState().setMessages(
    messages.map((m) => ({
      id: m.id,
      role: m.role as "user" | "assistant",
      content: m.content,
      created_at: m.created_at,
    })),
  );
}

export function subscribeCodexEvents(
  onChunk: (chunk: string) => void,
  onDone: () => void,
  onError: (message: string) => void,
  onPreviewUrl?: (url: string) => void,
) {
  const unsubs: Array<() => void> = [];
  listen<string>("codex-chunk", (e) => onChunk(e.payload)).then((u) =>
    unsubs.push(u),
  );
  listen("codex-done", () => onDone()).then((u) => unsubs.push(u));
  listen<string>("codex-error", (e) => onError(e.payload)).then((u) =>
    unsubs.push(u),
  );
  if (onPreviewUrl) {
    listen<string>("code-preview-url", (e) => onPreviewUrl(e.payload)).then(
      (u) => unsubs.push(u),
    );
  }
  return () => unsubs.forEach((u) => u());
}

export async function terminalOpen(cwd?: string, cols?: number, rows?: number) {
  return invoke<string>("terminal_open", { cwd: cwd ?? null, cols, rows });
}

export async function terminalWrite(id: string, data: string) {
  await invoke("terminal_write", { id, data });
}

export async function terminalResize(id: string, cols: number, rows: number) {
  await invoke("terminal_resize", { id, cols, rows });
}

export async function terminalClose(id: string) {
  await invoke("terminal_close", { id });
}

export function subscribeTerminal(
  onOutput: (id: string, data: string) => void,
  onExit: (id: string) => void,
) {
  const unsubs: Array<() => void> = [];
  listen<{ id: string; data: string }>("terminal-output", (e) =>
    onOutput(e.payload.id, e.payload.data),
  ).then((u) => unsubs.push(u));
  listen<{ id: string }>("terminal-exit", (e) => onExit(e.payload.id)).then(
    (u) => unsubs.push(u),
  );
  return () => unsubs.forEach((u) => u());
}

export interface SparkResponse {
  id: string;
  content: string;
  tags: string[];
  status: string;
  created_at: number;
  updated_at: number;
  last_nudged_at?: number | null;
  source_conversation_id?: string | null;
}

export async function loadSparks(status?: string) {
  return invoke<SparkResponse[]>("list_sparks", { status: status ?? null });
}

export async function createSpark(content: string, tags: string[]) {
  return invoke<SparkResponse>("create_spark", { content, tags });
}

export async function updateSpark(
  id: string,
  action: string,
  content?: string,
  tags?: string[],
) {
  return invoke<SparkResponse>("update_spark", {
    id,
    action,
    content: content ?? null,
    tags: tags ?? null,
  });
}

export async function deleteSpark(id: string) {
  await invoke("delete_spark", { id });
}

export async function fetchStaleSparkCount() {
  return invoke<number>("get_stale_spark_count");
}

export async function fetchStaleSparks() {
  return invoke<SparkResponse[]>("get_stale_sparks");
}

export function subscribeSparkEvents(
  onStale: (count: number) => void,
  onUpdated: () => void,
  onNavigateSpark: () => void,
) {
  const unsubs: Array<() => void> = [];
  listen<number>("sparks-stale", (e) => onStale(e.payload)).then((u) =>
    unsubs.push(u),
  );
  listen("sparks-updated", () => onUpdated()).then((u) => unsubs.push(u));
  listen("navigate-spark", () => onNavigateSpark()).then((u) =>
    unsubs.push(u),
  );
  return () => unsubs.forEach((u) => u());
}

export async function calendarListEvents(
  start: number,
  end: number,
  query?: string,
  categories?: string[],
): Promise<CalendarEvent[]> {
  return invoke<CalendarEvent[]>("calendar_list_events", {
    start,
    end,
    query: query ?? null,
    categories: categories ?? null,
  });
}

export async function calendarGetEvent(id: string): Promise<CalendarEvent> {
  return invoke<CalendarEvent>("calendar_get_event", { id });
}

export async function calendarCreateEvent(
  input: CreateEventInput,
): Promise<CalendarEvent> {
  return invoke<CalendarEvent>("calendar_create_event", { input });
}

export async function calendarUpdateEvent(
  id: string,
  input: UpdateEventInput,
): Promise<CalendarEvent> {
  return invoke<CalendarEvent>("calendar_update_event", { id, input });
}

export async function calendarDeleteEvent(id: string): Promise<void> {
  return invoke("calendar_delete_event", { id });
}

export async function calendarDuplicateEvent(
  id: string,
): Promise<CalendarEvent> {
  return invoke<CalendarEvent>("calendar_duplicate_event", { id });
}

export async function calendarSearchEvents(
  query: string,
  start?: number,
  end?: number,
): Promise<CalendarEvent[]> {
  return invoke<CalendarEvent[]>("calendar_search_events", {
    query,
    start: start ?? null,
    end: end ?? null,
  });
}

export async function calendarGetToday(): Promise<CalendarEvent[]> {
  return invoke<CalendarEvent[]>("calendar_get_today");
}

export async function calendarGetTomorrow(): Promise<CalendarEvent[]> {
  return invoke<CalendarEvent[]>("calendar_get_tomorrow");
}

export async function calendarGetThisWeek(): Promise<CalendarEvent[]> {
  return invoke<CalendarEvent[]>("calendar_get_this_week");
}

export async function calendarListNotifications(): Promise<ReminderDelivery[]> {
  return invoke<ReminderDelivery[]>("calendar_list_notifications");
}

export async function calendarSnoozeReminder(
  id: string,
  minutes: number,
): Promise<void> {
  return invoke("calendar_snooze_reminder", { id, minutes });
}

export async function calendarDismissReminder(id: string): Promise<void> {
  return invoke("calendar_dismiss_reminder", { id });
}

export async function calendarNotificationCount(): Promise<number> {
  return invoke<number>("calendar_notification_count");
}

export async function lifestyleListBlocks(
  start: number,
  end: number,
): Promise<ScheduleBlock[]> {
  return invoke<ScheduleBlock[]>("lifestyle_list_blocks", { start, end });
}

export async function lifestyleLastSleepDate(): Promise<string> {
  return invoke<string>("lifestyle_last_sleep_date");
}

export async function dreamList(sleepDate: string): Promise<DreamEntry[]> {
  return invoke<DreamEntry[]>("dream_list", { sleepDate });
}

export async function dreamLog(input: CreateDreamInput): Promise<DreamEntry> {
  return invoke<DreamEntry>("dream_log", { input });
}

export async function dreamUpdate(
  id: string,
  input: UpdateDreamInput,
): Promise<DreamEntry> {
  return invoke<DreamEntry>("dream_update", { id, input });
}

export async function dreamDelete(id: string): Promise<void> {
  return invoke("dream_delete", { id });
}

export async function dreamSearch(query: string): Promise<DreamEntry[]> {
  return invoke<DreamEntry[]>("dream_search", { query });
}

export async function workGetStats(): Promise<WorkStats> {
  return invoke<WorkStats>("work_get_stats");
}

export async function workLogSales(
  amount: number,
  workDate?: string | null,
  currency?: string | null,
): Promise<WorkDayLog> {
  return invoke<WorkDayLog>("work_log_sales", {
    amount,
    workDate: workDate ?? null,
    currency: currency ?? null,
  });
}

export async function workSetHours(
  workDate: string | null | undefined,
  actualStartMs: number | null | undefined,
  actualEndMs: number | null | undefined,
): Promise<WorkDayLog> {
  return invoke<WorkDayLog>("work_set_hours", {
    workDate: workDate ?? null,
    actualStartMs: actualStartMs ?? null,
    actualEndMs: actualEndMs ?? null,
  });
}

export async function workGetDayLog(workDate: string): Promise<WorkDayLog> {
  return invoke<WorkDayLog>("work_get_day_log", { workDate });
}

export async function subscribeCalendarEvents(
  onUpdated: () => void,
): Promise<() => void> {
  const unsub = await listen("calendar-updated", () => onUpdated());
  return () => unsub();
}

export async function subscribeCalendarReminders(
  onReminder: (delivery: ReminderDelivery) => void,
  onCount: (count: number) => void,
): Promise<() => void> {
  const unsubs: Array<() => void> = [];
  unsubs.push(
    await listen<ReminderDelivery>("calendar-reminder", (e) =>
      onReminder(e.payload),
    ),
  );
  unsubs.push(
    await listen<number>("calendar-notification-count", (e) =>
      onCount(e.payload),
    ),
  );
  return () => unsubs.forEach((u) => u());
}

