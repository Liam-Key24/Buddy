import { useEffect, useState } from "react";
import {
  ArrowsClockwise,
  FloppyDisk,
  Plus,
  Trash,
} from "@phosphor-icons/react";
import { useSettingsStore } from "../stores/useSettingsStore";
import {
  deleteSecret,
  getSecretStatus,
  loadSettings,
  refreshCache,
  saveSetting,
  setExcludedPaths,
  setSecret,
} from "../lib/api";

const SECRET_KEYS: { key: string; label: string }[] = [
  { key: "openai_api_key", label: "OpenAI" },
  { key: "cursor_api_key", label: "Cursor" },
  { key: "smtp_password", label: "Email / SMTP" },
];

export function Settings() {
  const settings = useSettingsStore();
  const [saved, setSaved] = useState(false);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    loadSettings().then((s) => {
      settings.setSettings({
        mlxUrl: s.mlx_url,
        brainUrl: s.brain_url,
        modelName: s.model_name,
        logLevel: s.log_level,
        autoStartMlx: s.auto_start_mlx,
        modelNameChat: s.model_name_chat,
        modelNameCode: s.model_name_code,
        llmProfileRouter: s.llm_profile_router,
        codexModel: s.codex_model,
        codexWorkspace: s.codex_workspace,
        codeAgentBackend: s.code_agent_backend,
        codeModel: s.code_model,
        cursorPath: s.cursor_path,
        codexPath: s.codex_path,
        emailSignature: s.email_signature,
        emailGreeting: s.email_greeting,
        emailBodyTemplate: s.email_body_template,
        fsExcludedPaths: s.fs_excluded_paths,
        calendarNotificationsEnabled: s.calendar_notifications_enabled ?? true,
        calendarDefaultTimezone: s.calendar_default_timezone ?? "UTC",
        calendarDefaultRemindersJson:
          s.calendar_default_reminders_json ??
          JSON.stringify([{ minutes_before: 15, method: "popup" }]),
      });
    });
  }, []);

  async function handleSave() {
    setSaving(true);
    try {
      await Promise.all([
        saveSetting("log_level", settings.logLevel),
        saveSetting("auto_start_mlx", String(settings.autoStartMlx)),
        saveSetting("model_name", settings.modelName),
        saveSetting("model_name_chat", settings.modelName),
        saveSetting("model_name_code", settings.modelName),
        saveSetting("llm_profile_router", settings.modelName),
        saveSetting("codex_model", settings.codexModel),
        saveSetting("codex_workspace", settings.codexWorkspace),
        saveSetting("code_agent_backend", settings.codeAgentBackend),
        saveSetting("code_model", settings.codeModel),
        saveSetting("email_greeting", settings.emailGreeting),
        saveSetting("email_signature", settings.emailSignature),
        saveSetting(
          "calendar_notifications_enabled",
          String(settings.calendarNotificationsEnabled),
        ),
        saveSetting(
          "calendar_default_timezone",
          settings.calendarDefaultTimezone || "UTC",
        ),
        setExcludedPaths(settings.fsExcludedPaths),
      ]);
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="flex-1 overflow-y-auto p-6">
      <div className="mx-auto max-w-xl space-y-8 pb-16">
        <div className="flex items-center justify-between gap-4">
          <div>
            <h1 className="text-lg font-semibold text-zinc-100">Settings</h1>
            <p className="text-xs text-zinc-500">
              Models, keys, and a few preferences.
            </p>
          </div>
          <button
            onClick={handleSave}
            disabled={saving}
            className="flex items-center gap-2 rounded-xl bg-blue-500 px-4 py-2 text-sm font-medium text-white transition hover:bg-blue-600 disabled:opacity-50"
          >
            <FloppyDisk size={16} weight="duotone" />
            {saved ? "Saved" : saving ? "Saving…" : "Save"}
          </button>
        </div>

        <Section title="Models">
          <Field
            label="Local model"
            value={settings.modelName}
            onChange={(v) => settings.setSettings({ modelName: v })}
          />
          <div>
            <label className="mb-1 block text-xs font-medium text-zinc-400">
              Code agent
            </label>
            <select
              value={settings.codeAgentBackend}
              onChange={(e) =>
                settings.setSettings({ codeAgentBackend: e.target.value })
              }
              className="w-full rounded-xl border border-zinc-700 bg-zinc-800 px-3 py-2 text-sm text-zinc-200 outline-none focus:border-blue-500 focus:ring-2 focus:ring-blue-500/20"
            >
              <option value="cursor">Cursor</option>
              <option value="codex">Codex</option>
            </select>
          </div>
          {settings.codeAgentBackend === "cursor" ? (
            <Field
              label="Cursor model"
              value={settings.codeModel}
              onChange={(v) => settings.setSettings({ codeModel: v })}
            />
          ) : (
            <Field
              label="Codex model"
              value={settings.codexModel}
              onChange={(v) => settings.setSettings({ codexModel: v })}
            />
          )}
          <Field
            label="Project folder"
            value={settings.codexWorkspace}
            onChange={(v) => settings.setSettings({ codexWorkspace: v })}
          />
          <label className="flex items-center gap-2 text-sm text-zinc-300">
            <input
              type="checkbox"
              checked={settings.autoStartMlx}
              onChange={(e) =>
                settings.setSettings({ autoStartMlx: e.target.checked })
              }
              className="rounded border-zinc-600 bg-zinc-800 text-blue-500 focus:ring-blue-500/20"
            />
            Auto-start local model server
          </label>
        </Section>

        <Section title="API keys">
          <p className="text-xs text-zinc-500">
            Stored in the macOS Keychain.
          </p>
          <ApiKeysSection />
        </Section>

        <Section title="Email">
          <Field
            label="Greeting"
            value={settings.emailGreeting}
            onChange={(v) => settings.setSettings({ emailGreeting: v })}
          />
          <div>
            <label className="mb-1 block text-xs font-medium text-zinc-400">
              Signature
            </label>
            <textarea
              value={settings.emailSignature}
              onChange={(e) =>
                settings.setSettings({ emailSignature: e.target.value })
              }
              rows={3}
              className="w-full resize-none rounded-xl border border-zinc-700 bg-zinc-800 px-3 py-2 text-sm text-zinc-200 outline-none focus:border-blue-500 focus:ring-2 focus:ring-blue-500/20"
            />
          </div>
        </Section>

        <Section title="Calendar">
          <label className="flex items-center gap-2 text-sm text-zinc-300">
            <input
              type="checkbox"
              checked={settings.calendarNotificationsEnabled}
              onChange={(e) =>
                settings.setSettings({
                  calendarNotificationsEnabled: e.target.checked,
                })
              }
              className="rounded border-zinc-600 bg-zinc-800 text-blue-500 focus:ring-blue-500/20"
            />
            Desktop reminder notifications
          </label>
          <Field
            label="Timezone"
            value={settings.calendarDefaultTimezone}
            onChange={(v) =>
              settings.setSettings({ calendarDefaultTimezone: v })
            }
          />
        </Section>

        <Section title="Filesystem">
          <p className="text-xs text-zinc-500">
            Paths under your home folder that Buddy should not touch.
          </p>
          <FilesystemSection />
        </Section>

        <Section title="Cache">
          <CacheSection />
        </Section>
      </div>
    </div>
  );
}

function Section({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  return (
    <section className="space-y-3">
      <h2 className="text-xs font-semibold uppercase tracking-wide text-zinc-500">
        {title}
      </h2>
      <div className="space-y-3 rounded-2xl border border-zinc-800 bg-zinc-900 p-4">
        {children}
      </div>
    </section>
  );
}

function ApiKeysSection() {
  const [status, setStatus] = useState<Record<string, boolean>>({});
  const [values, setValues] = useState<Record<string, string>>({});
  const [message, setMessage] = useState<string | null>(null);

  async function refresh() {
    setStatus(await getSecretStatus());
  }

  useEffect(() => {
    refresh().catch(console.error);
  }, []);

  async function handleSave(key: string) {
    const value = values[key];
    if (!value) return;
    await setSecret(key, value);
    setValues((v) => ({ ...v, [key]: "" }));
    setMessage("Saved");
    setTimeout(() => setMessage(null), 2000);
    await refresh();
  }

  async function handleClear(key: string) {
    await deleteSecret(key);
    await refresh();
  }

  return (
    <div className="space-y-3">
      {SECRET_KEYS.map(({ key, label }) => (
        <div key={key}>
          <label className="mb-1 flex items-center justify-between text-xs font-medium text-zinc-400">
            <span>{label}</span>
            <span
              className={status[key] ? "text-emerald-400" : "text-zinc-600"}
            >
              {status[key] ? "set" : "not set"}
            </span>
          </label>
          <div className="flex gap-2">
            <input
              type="password"
              value={values[key] ?? ""}
              placeholder={status[key] ? "••••••••" : "Enter key"}
              onChange={(e) =>
                setValues((v) => ({ ...v, [key]: e.target.value }))
              }
              className="flex-1 rounded-xl border border-zinc-700 bg-zinc-800 px-3 py-2 text-sm text-zinc-200 outline-none focus:border-blue-500 focus:ring-2 focus:ring-blue-500/20"
            />
            <button
              onClick={() => handleSave(key)}
              disabled={!values[key]}
              className="rounded-xl bg-blue-500 px-3 py-2 text-xs font-medium text-white transition hover:bg-blue-600 disabled:opacity-40"
            >
              Save
            </button>
            {status[key] && (
              <button
                onClick={() => handleClear(key)}
                className="rounded-xl border border-zinc-700 px-3 py-2 text-xs font-medium text-zinc-400 transition hover:border-rose-500 hover:text-rose-400"
              >
                Clear
              </button>
            )}
          </div>
        </div>
      ))}
      {message && <p className="text-xs text-emerald-400">{message}</p>}
    </div>
  );
}

function FilesystemSection() {
  const settings = useSettingsStore();
  const [newPath, setNewPath] = useState("");

  function addPath() {
    const trimmed = newPath.trim();
    if (!trimmed || settings.fsExcludedPaths.includes(trimmed)) return;
    settings.setSettings({
      fsExcludedPaths: [...settings.fsExcludedPaths, trimmed],
    });
    setNewPath("");
  }

  function removePath(path: string) {
    settings.setSettings({
      fsExcludedPaths: settings.fsExcludedPaths.filter((p) => p !== path),
    });
  }

  return (
    <div className="space-y-3">
      <div className="space-y-1.5">
        {settings.fsExcludedPaths.map((path) => (
          <div
            key={path}
            className="flex items-center justify-between rounded-lg bg-zinc-800 px-3 py-2 text-sm text-zinc-300"
          >
            <span className="truncate">{path}</span>
            <button
              onClick={() => removePath(path)}
              className="ml-2 shrink-0 text-zinc-500 transition hover:text-rose-400"
            >
              <Trash size={14} />
            </button>
          </div>
        ))}
        {settings.fsExcludedPaths.length === 0 && (
          <p className="text-xs text-zinc-600">No exclusions.</p>
        )}
      </div>
      <div className="flex gap-2">
        <input
          type="text"
          value={newPath}
          placeholder="e.g. Documents/private"
          onChange={(e) => setNewPath(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && addPath()}
          className="flex-1 rounded-xl border border-zinc-700 bg-zinc-800 px-3 py-2 text-sm text-zinc-200 outline-none focus:border-blue-500 focus:ring-2 focus:ring-blue-500/20"
        />
        <button
          onClick={addPath}
          className="flex items-center gap-1 rounded-xl bg-blue-500 px-3 py-2 text-xs font-medium text-white transition hover:bg-blue-600"
        >
          <Plus size={14} weight="bold" />
          Add
        </button>
      </div>
    </div>
  );
}

function CacheSection() {
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<string | null>(null);

  async function handleRefresh() {
    setRunning(true);
    setResult(null);
    try {
      const r = await refreshCache();
      setResult(
        `Reindexed ${r.memories_reindexed} memories in ${r.duration_ms}ms`,
      );
    } catch (err) {
      setResult(`Failed: ${err}`);
    } finally {
      setRunning(false);
    }
  }

  return (
    <div className="space-y-3">
      <p className="text-xs text-zinc-500">
        Rebuild the semantic index and workspace profile.
      </p>
      <button
        onClick={handleRefresh}
        disabled={running}
        className="flex items-center gap-2 rounded-xl border border-zinc-700 px-4 py-2 text-sm font-medium text-zinc-200 transition hover:border-zinc-500 hover:bg-zinc-800 disabled:opacity-50"
      >
        <ArrowsClockwise
          size={16}
          weight="bold"
          className={running ? "animate-spin" : ""}
        />
        {running ? "Refreshing…" : "Refresh cache"}
      </button>
      {result && <p className="text-xs text-zinc-400">{result}</p>}
    </div>
  );
}

function Field({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
}) {
  return (
    <div>
      <label className="mb-1 block text-xs font-medium text-zinc-400">
        {label}
      </label>
      <input
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="w-full rounded-xl border border-zinc-700 bg-zinc-800 px-3 py-2 text-sm text-zinc-200 outline-none focus:border-blue-500 focus:ring-2 focus:ring-blue-500/20"
      />
    </div>
  );
}
