import { useEffect, useState } from "react";
import {
  ArrowsClockwise,
  FloppyDisk,
  Plus,
  Trash,
} from "@phosphor-icons/react";
import { useSettingsStore } from "../stores/useSettingsStore";
import {
  ExternalAction,
  deleteSecret,
  getSecretStatus,
  listExternalActions,
  loadSettings,
  refreshCache,
  saveSetting,
  setExcludedPaths,
  setSecret,
} from "../lib/api";

type Tab = "general" | "keys" | "filesystem" | "email" | "actions" | "cache";

const TABS: { id: Tab; label: string }[] = [
  { id: "general", label: "General" },
  { id: "keys", label: "API Keys" },
  { id: "filesystem", label: "Filesystem" },
  { id: "email", label: "Email" },
  { id: "actions", label: "External Actions" },
  { id: "cache", label: "Cache" },
];

const SECRET_KEYS: { key: string; label: string }[] = [
  { key: "openai_api_key", label: "OpenAI API Key (Codex / GPT-5.5)" },
  { key: "cursor_api_key", label: "Cursor API Key" },
  { key: "smtp_password", label: "Email / SMTP Password" },
];

export function Settings() {
  const settings = useSettingsStore();
  const [tab, setTab] = useState<Tab>("general");
  const [saved, setSaved] = useState(false);

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
      });
    });
  }, []);

  async function handleSaveGeneral() {
    await Promise.all([
      saveSetting("log_level", settings.logLevel),
      saveSetting("auto_start_mlx", String(settings.autoStartMlx)),
      saveSetting("model_name", settings.modelName),
      saveSetting("model_name_chat", settings.modelNameChat),
      saveSetting("model_name_code", settings.modelNameCode),
      saveSetting("llm_profile_router", settings.llmProfileRouter),
      saveSetting("codex_model", settings.codexModel),
      saveSetting("codex_workspace", settings.codexWorkspace),
      saveSetting("code_agent_backend", settings.codeAgentBackend),
      saveSetting("code_model", settings.codeModel),
      saveSetting("cursor_path", settings.cursorPath),
      saveSetting("codex_path", settings.codexPath),
    ]);
    flashSaved();
  }

  function flashSaved() {
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  }

  return (
    <div className="flex-1 overflow-y-auto p-6">
      <div className="mx-auto max-w-2xl space-y-4">
        <div className="flex flex-wrap gap-1 rounded-xl border border-zinc-800 bg-zinc-900 p-1">
          {TABS.map((t) => (
            <button
              key={t.id}
              onClick={() => setTab(t.id)}
              className={`rounded-lg px-3 py-1.5 text-xs font-medium transition ${
                tab === t.id
                  ? "bg-blue-500 text-white"
                  : "text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200"
              }`}
            >
              {t.label}
            </button>
          ))}
        </div>

        <div className="rounded-2xl border border-zinc-800 bg-zinc-900 p-6 shadow-sm">
          {tab === "general" && (
            <div className="space-y-4">
              <Field label="MLX URL" value={settings.mlxUrl} readOnly />
              <Field label="Brain URL" value={settings.brainUrl} readOnly />
              <Field
                label="Default Model"
                value={settings.modelName}
                onChange={(v) => settings.setSettings({ modelName: v })}
              />
              <Field
                label="Chat Model"
                value={settings.modelNameChat}
                onChange={(v) => settings.setSettings({ modelNameChat: v })}
              />
              <Field
                label="Code Model"
                value={settings.modelNameCode}
                onChange={(v) => settings.setSettings({ modelNameCode: v })}
              />
              <Field
                label="Router Model"
                value={settings.llmProfileRouter}
                onChange={(v) =>
                  settings.setSettings({ llmProfileRouter: v })
                }
              />
              <div>
                <label className="mb-1 block text-xs font-medium text-zinc-400">
                  Code Agent Backend
                </label>
                <select
                  value={settings.codeAgentBackend}
                  onChange={(e) =>
                    settings.setSettings({ codeAgentBackend: e.target.value })
                  }
                  className="w-full rounded-xl border border-zinc-700 bg-zinc-800 px-3 py-2 text-sm text-zinc-200 outline-none focus:border-blue-500 focus:ring-2 focus:ring-blue-500/20"
                >
                  <option value="cursor">Cursor (cursor-agent)</option>
                  <option value="codex">Codex (codex CLI)</option>
                </select>
              </div>
              <Field
                label="Cursor Model (code)"
                value={settings.codeModel}
                onChange={(v) => settings.setSettings({ codeModel: v })}
              />
              <Field
                label="Cursor CLI Path (leave blank to use PATH)"
                value={settings.cursorPath}
                onChange={(v) => settings.setSettings({ cursorPath: v })}
              />
              <Field
                label="Codex Model"
                value={settings.codexModel}
                onChange={(v) => settings.setSettings({ codexModel: v })}
              />
              <Field
                label="Codex CLI Path (leave blank to use PATH)"
                value={settings.codexPath}
                onChange={(v) => settings.setSettings({ codexPath: v })}
              />
              <Field
                label="Project Base Folder"
                value={settings.codexWorkspace}
                onChange={(v) => settings.setSettings({ codexWorkspace: v })}
              />
              <div>
                <label className="mb-1 block text-xs font-medium text-zinc-400">
                  Log Level
                </label>
                <select
                  value={settings.logLevel}
                  onChange={(e) =>
                    settings.setSettings({ logLevel: e.target.value })
                  }
                  className="w-full rounded-xl border border-zinc-700 bg-zinc-800 px-3 py-2 text-sm text-zinc-200 outline-none focus:border-blue-500 focus:ring-2 focus:ring-blue-500/20"
                >
                  <option value="debug">debug</option>
                  <option value="info">info</option>
                  <option value="warn">warn</option>
                  <option value="error">error</option>
                </select>
              </div>
              <label className="flex items-center gap-2 text-sm text-zinc-300">
                <input
                  type="checkbox"
                  checked={settings.autoStartMlx}
                  onChange={(e) =>
                    settings.setSettings({ autoStartMlx: e.target.checked })
                  }
                  className="rounded border-zinc-600 bg-zinc-800 text-blue-500 focus:ring-blue-500/20"
                />
                Auto-start MLX
              </label>
              <SaveButton onClick={handleSaveGeneral} saved={saved} />
            </div>
          )}

          {tab === "keys" && <ApiKeysSection />}
          {tab === "filesystem" && <FilesystemSection />}
          {tab === "email" && <EmailSection onSaved={flashSaved} saved={saved} />}
          {tab === "actions" && <ExternalActionsSection />}
          {tab === "cache" && <CacheSection />}
        </div>
      </div>
    </div>
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
    setMessage(`Saved ${key}`);
    setTimeout(() => setMessage(null), 2000);
    await refresh();
  }

  async function handleClear(key: string) {
    await deleteSecret(key);
    await refresh();
  }

  return (
    <div className="space-y-4">
      <p className="text-xs text-zinc-500">
        Keys are stored in the macOS Keychain, never in the app database.
      </p>
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
  const [saved, setSaved] = useState(false);

  async function persist(paths: string[]) {
    settings.setSettings({ fsExcludedPaths: paths });
    await setExcludedPaths(paths);
    setSaved(true);
    setTimeout(() => setSaved(false), 1500);
  }

  function addPath() {
    const trimmed = newPath.trim();
    if (!trimmed || settings.fsExcludedPaths.includes(trimmed)) return;
    persist([...settings.fsExcludedPaths, trimmed]);
    setNewPath("");
  }

  function removePath(path: string) {
    persist(settings.fsExcludedPaths.filter((p) => p !== path));
  }

  return (
    <div className="space-y-4">
      <p className="text-xs text-zinc-500">
        Buddy can read and edit files under your home folder, except these
        excluded locations. Entries are names or paths relative to your home
        folder.
      </p>
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
          <p className="text-xs text-zinc-600">No exclusions configured.</p>
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
      {saved && <p className="text-xs text-emerald-400">Saved</p>}
    </div>
  );
}

function EmailSection({
  onSaved,
  saved,
}: {
  onSaved: () => void;
  saved: boolean;
}) {
  const settings = useSettingsStore();

  async function handleSave() {
    await Promise.all([
      saveSetting("email_greeting", settings.emailGreeting),
      saveSetting("email_signature", settings.emailSignature),
      saveSetting("email_body_template", settings.emailBodyTemplate),
    ]);
    onSaved();
  }

  const preview = settings.emailBodyTemplate
    .replace("{greeting}", settings.emailGreeting.replace("{name}", "Alex"))
    .replace("{name}", "Alex")
    .replace("{body}", "This is the message body.")
    .replace("{signature}", settings.emailSignature);

  return (
    <div className="space-y-4">
      <Field
        label="Greeting (use {name} for recipient)"
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
      <div>
        <label className="mb-1 block text-xs font-medium text-zinc-400">
          Body Template ({"{greeting}"}, {"{body}"}, {"{signature}"})
        </label>
        <textarea
          value={settings.emailBodyTemplate}
          onChange={(e) =>
            settings.setSettings({ emailBodyTemplate: e.target.value })
          }
          rows={4}
          className="w-full resize-none rounded-xl border border-zinc-700 bg-zinc-800 px-3 py-2 font-mono text-xs text-zinc-200 outline-none focus:border-blue-500 focus:ring-2 focus:ring-blue-500/20"
        />
      </div>
      <div>
        <label className="mb-1 block text-xs font-medium text-zinc-400">
          Preview
        </label>
        <pre className="whitespace-pre-wrap rounded-xl border border-zinc-800 bg-zinc-950 p-3 text-xs text-zinc-300">
          {preview}
        </pre>
      </div>
      <SaveButton onClick={handleSave} saved={saved} />
    </div>
  );
}

function ExternalActionsSection() {
  const [actions, setActions] = useState<ExternalAction[]>([]);

  useEffect(() => {
    listExternalActions(100).then(setActions).catch(console.error);
  }, []);

  if (actions.length === 0) {
    return <p className="text-xs text-zinc-500">No external actions yet.</p>;
  }

  return (
    <div className="space-y-2">
      {actions.map((a) => (
        <div
          key={a.id}
          className="rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2"
        >
          <div className="flex items-center justify-between">
            <span className="text-xs font-medium text-zinc-300">
              {a.action_type}
            </span>
            <span
              className={`text-[10px] ${
                a.approved ? "text-emerald-400" : "text-amber-400"
              }`}
            >
              {a.approved ? "approved" : "pending"}
            </span>
          </div>
          <p className="mt-0.5 text-xs text-zinc-500">{a.summary}</p>
          <p className="mt-0.5 text-[10px] text-zinc-600">
            {new Date(a.created_at).toLocaleString()}
          </p>
        </div>
      ))}
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
    <div className="space-y-4">
      <p className="text-xs text-zinc-500">
        Rebuild the semantic index and refresh the workspace profile.
      </p>
      <button
        onClick={handleRefresh}
        disabled={running}
        className="flex items-center gap-2 rounded-xl bg-blue-500 px-4 py-2 text-sm font-medium text-white transition hover:bg-blue-600 disabled:opacity-50"
      >
        <ArrowsClockwise
          size={16}
          weight="bold"
          className={running ? "animate-spin" : ""}
        />
        {running ? "Refreshing..." : "Refresh cache"}
      </button>
      {result && <p className="text-xs text-zinc-400">{result}</p>}
    </div>
  );
}

function SaveButton({
  onClick,
  saved,
}: {
  onClick: () => void;
  saved: boolean;
}) {
  return (
    <button
      onClick={onClick}
      className="flex items-center gap-2 rounded-xl bg-blue-500 px-4 py-2 text-sm font-medium text-white transition hover:bg-blue-600"
    >
      <FloppyDisk size={16} weight="duotone" />
      {saved ? "Saved" : "Save"}
    </button>
  );
}

function Field({
  label,
  value,
  onChange,
  readOnly,
}: {
  label: string;
  value: string;
  onChange?: (v: string) => void;
  readOnly?: boolean;
}) {
  return (
    <div>
      <label className="mb-1 block text-xs font-medium text-zinc-400">
        {label}
      </label>
      <input
        type="text"
        value={value}
        readOnly={readOnly}
        onChange={(e) => onChange?.(e.target.value)}
        className="w-full rounded-xl border border-zinc-700 bg-zinc-800 px-3 py-2 text-sm text-zinc-200 outline-none focus:border-blue-500 focus:ring-2 focus:ring-blue-500/20 read-only:opacity-60"
      />
    </div>
  );
}
