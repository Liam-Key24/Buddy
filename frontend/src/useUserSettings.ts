import { useEffect, useRef, useState } from "react";
import {
  fetchSettings,
  updateSettings,
  type UserSettings,
  type WorkSettings,
} from "./api";

/** Live preference: confirm before deleting goals/events. Defaults true. */
export function useConfirmDeletes(): boolean {
  const [value, setValue] = useState(true);
  useEffect(() => {
    let cancelled = false;
    fetchSettings()
      .then((s) => {
        if (!cancelled) setValue(s.confirm_deletes);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, []);
  return value;
}

export type SettingsPatch = Partial<Omit<UserSettings, "work">> & {
  work?: Partial<WorkSettings> | WorkSettings;
};

/** Load + debounced save for settings pages. */
export function useSettingsForm(debounceMs = 300) {
  const [settings, setSettings] = useState<UserSettings | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const saveTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    let cancelled = false;
    fetchSettings()
      .then((row) => {
        if (!cancelled) setSettings(row);
      })
      .catch((e: Error) => {
        if (!cancelled) setError(e.message);
      });
    return () => {
      cancelled = true;
      if (saveTimer.current) clearTimeout(saveTimer.current);
    };
  }, []);

  function persist(patch: SettingsPatch) {
    setSettings((prev) => {
      if (!prev) return prev;
      return {
        ...prev,
        ...patch,
        work: patch.work ? { ...prev.work, ...patch.work } : prev.work,
      };
    });
    if (saveTimer.current) clearTimeout(saveTimer.current);
    saveTimer.current = setTimeout(() => {
      setSaving(true);
      updateSettings(patch)
        .then((row) => setSettings(row))
        .catch((e: Error) => setError(e.message))
        .finally(() => setSaving(false));
    }, debounceMs);
  }

  return { settings, error, saving, persist, setError };
}
