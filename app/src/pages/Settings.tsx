import { useEffect, useState, type ComponentType } from "react";
import {
  ArrowsClockwise,
  Barbell,
  Bell,
  Brain,
  ChatTeardrop,
  Code,
  Cpu,
  EnvelopeSimple,
  FloppyDisk,
  Folder,
  FolderMinus,
  Gear,
  Globe,
  HardDrives,
  Key,
  PenNib,
  Plus,
  Sparkle,
  Terminal,
  Trash,
  Wallet,
} from "@phosphor-icons/react";
import { useSettingsStore } from "../stores/useSettingsStore";
import { useAppStore } from "../stores/useAppStore";
import {
  deleteSecret,
  getSecretStatus,
  loadSettings,
  refreshCache,
  restartBrain,
  restartMlx,
  saveSetting,
  setExcludedPaths,
  setSecret,
} from "../lib/api";

type IconComp = ComponentType<{
  size?: number;
  weight?: "fill" | "regular" | "duotone" | "bold";
  className?: string;
}>;

const SECRET_KEYS: { key: string; Icon: IconComp; title: string }[] = [
  { key: "openai_api_key", Icon: Sparkle, title: "OpenAI" },
  { key: "cursor_api_key", Icon: Code, title: "Cursor" },
  { key: "smtp_password", Icon: EnvelopeSimple, title: "Email" },
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
        fitnessCalorieTarget: s.fitness_calorie_target ?? "2500",
        moneyCurrency: s.money_currency ?? "GBP",
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
        saveSetting(
          "model_name_chat",
          settings.modelNameChat || "mlx-community/Llama-3.2-3B-Instruct-4bit",
        ),
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
        saveSetting("fitness_calorie_target", settings.fitnessCalorieTarget || "2500"),
        saveSetting("money_currency", settings.moneyCurrency || "GBP"),
        setExcludedPaths(settings.fsExcludedPaths),
      ]);
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="flex-1 overflow-y-auto p-4">
      <div className="mx-auto max-w-xl space-y-8 pb-16">
        <div className="flex items-center gap-2">
          <Gear size={18} weight="fill" className="text-blue-400" />
          <h3 className="min-w-0 flex-1 text-sm font-medium text-zinc-200">Settings</h3>
          <button
            type="button"
            title={saved ? "Saved" : saving ? "Saving" : "Save"}
            aria-label="Save"
            onClick={() => void handleSave()}
            disabled={saving}
            className="rounded-md p-1 text-blue-400 hover:bg-zinc-800 disabled:opacity-40"
          >
            <FloppyDisk size={16} weight={saved ? "fill" : "bold"} />
          </button>
        </div>

        <Section icon={HardDrives} title="Services" hint="Tap to restart.">
          <ServicesSection />
        </Section>

        <Section icon={Sparkle} title="Models" hint="Local chat model and code backend.">
          <IconField
            icon={Cpu}
            value={settings.modelName}
            onChange={(v) => settings.setSettings({ modelName: v })}
            placeholder="mlx-community/…"
          />
          <div className="flex items-center gap-2">
            <Code size={18} className="shrink-0 text-zinc-500" />
            <IconChoice
              active={settings.codeAgentBackend === "cursor"}
              title="Cursor"
              onClick={() => settings.setSettings({ codeAgentBackend: "cursor" })}
            >
              <Code size={16} weight={settings.codeAgentBackend === "cursor" ? "fill" : "regular"} />
            </IconChoice>
            <IconChoice
              active={settings.codeAgentBackend === "codex"}
              title="Codex"
              onClick={() => settings.setSettings({ codeAgentBackend: "codex" })}
            >
              <Terminal size={16} weight={settings.codeAgentBackend === "codex" ? "fill" : "regular"} />
            </IconChoice>
            <input
              value={
                settings.codeAgentBackend === "cursor"
                  ? settings.codeModel
                  : settings.codexModel
              }
              onChange={(e) =>
                settings.setSettings(
                  settings.codeAgentBackend === "cursor"
                    ? { codeModel: e.target.value }
                    : { codexModel: e.target.value },
                )
              }
              placeholder="Model"
              className="min-w-0 flex-1 bg-transparent text-sm text-zinc-100 outline-none placeholder:text-zinc-600"
            />
          </div>
          <IconField
            icon={Folder}
            value={settings.codexWorkspace}
            onChange={(v) => settings.setSettings({ codexWorkspace: v })}
            placeholder="Project folder"
          />
          <div className="flex items-center justify-between gap-3">
            <span title="Keep model loaded">
              <Cpu size={18} className="text-zinc-500" />
            </span>
            <Toggle
              on={settings.autoStartMlx}
              onChange={(v) => settings.setSettings({ autoStartMlx: v })}
            />
          </div>
        </Section>

        <Section icon={Key} title="Keys" hint="macOS Keychain.">
          <ApiKeysSection />
        </Section>

        <Section icon={EnvelopeSimple} title="Email" hint="Greeting and sign-off.">
          <IconField
            icon={ChatTeardrop}
            value={settings.emailGreeting}
            onChange={(v) => settings.setSettings({ emailGreeting: v })}
            placeholder="Hi,"
          />
          <div className="flex items-start gap-2.5">
            <PenNib size={18} className="mt-2 shrink-0 text-zinc-500" />
            <textarea
              value={settings.emailSignature}
              onChange={(e) => settings.setSettings({ emailSignature: e.target.value })}
              rows={3}
              placeholder="Signature"
              className="w-full resize-none bg-transparent text-sm text-zinc-100 outline-none placeholder:text-zinc-600"
            />
          </div>
        </Section>

        <Section icon={Bell} title="Calendar" hint="Reminders and timezone.">
          <div className="flex items-center justify-between gap-3">
            <Bell size={18} className="text-zinc-500" />
            <Toggle
              on={settings.calendarNotificationsEnabled}
              onChange={(v) =>
                settings.setSettings({ calendarNotificationsEnabled: v })
              }
            />
          </div>
          <IconField
            icon={Globe}
            value={settings.calendarDefaultTimezone}
            onChange={(v) => settings.setSettings({ calendarDefaultTimezone: v })}
            placeholder="Europe/London"
          />
        </Section>

        <Section icon={Barbell} title="Life" hint="Daily target and currency.">
          <IconField
            icon={Barbell}
            value={settings.fitnessCalorieTarget}
            onChange={(v) => settings.setSettings({ fitnessCalorieTarget: v })}
            placeholder="2500"
          />
          <div className="flex items-center gap-2">
            <Wallet size={18} className="shrink-0 text-zinc-500" />
            {(["GBP", "USD", "EUR"] as const).map((c) => (
              <button
                key={c}
                type="button"
                onClick={() => settings.setSettings({ moneyCurrency: c })}
                className={`rounded-md px-2 py-1 text-xs ${
                  settings.moneyCurrency === c
                    ? "text-blue-400"
                    : "text-zinc-600 hover:text-zinc-300"
                }`}
              >
                {c}
              </button>
            ))}
          </div>
        </Section>

        <Section icon={FolderMinus} title="Ignore" hint="Home paths Buddy won’t touch.">
          <FilesystemSection />
        </Section>

        <Section icon={ArrowsClockwise} title="Cache" hint="Rebuild memory index.">
          <CacheSection />
        </Section>
      </div>
    </div>
  );
}

function Section({
  icon: SectionIcon,
  title,
  hint,
  children,
}: {
  icon: IconComp;
  title: string;
  hint: string;
  children: React.ReactNode;
}) {
  return (
    <section>
      <div className="mb-1 flex items-center gap-2">
        <SectionIcon size={18} weight="duotone" className="text-zinc-400" />
        <h2 className="text-sm font-medium text-zinc-200">{title}</h2>
      </div>
      <p className="mb-2 text-[11px] text-zinc-500">{hint}</p>
      <div className="space-y-3 rounded-2xl border border-zinc-800 bg-zinc-900 p-4">
        {children}
      </div>
    </section>
  );
}

function IconField({
  icon: FieldIcon,
  value,
  onChange,
  placeholder,
}: {
  icon: IconComp;
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
}) {
  return (
    <div className="flex items-center gap-2.5">
      <FieldIcon size={18} weight="regular" className="shrink-0 text-zinc-500" />
      <input
        type="text"
        value={value}
        placeholder={placeholder}
        onChange={(e) => onChange(e.target.value)}
        className="min-w-0 flex-1 bg-transparent text-sm text-zinc-100 outline-none placeholder:text-zinc-600"
      />
    </div>
  );
}

function IconChoice({
  active,
  title,
  onClick,
  children,
}: {
  active: boolean;
  title: string;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      title={title}
      aria-pressed={active}
      onClick={onClick}
      className={`rounded-md p-1.5 ${active ? "text-blue-400" : "text-zinc-600 hover:text-zinc-300"}`}
    >
      {children}
    </button>
  );
}

function Toggle({ on, onChange }: { on: boolean; onChange: (v: boolean) => void }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      onClick={() => onChange(!on)}
      className={`relative h-6 w-10 shrink-0 rounded-full transition ${
        on ? "bg-blue-500" : "bg-zinc-700"
      }`}
    >
      <span
        className={`absolute top-0.5 left-0.5 h-5 w-5 rounded-full bg-white transition ${
          on ? "translate-x-4" : "translate-x-0"
        }`}
      />
    </button>
  );
}

function ApiKeysSection() {
  const [status, setStatus] = useState<Record<string, boolean>>({});
  const [values, setValues] = useState<Record<string, string>>({});

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
    await refresh();
  }

  async function handleClear(key: string) {
    await deleteSecret(key);
    await refresh();
  }

  return (
    <div className="space-y-3">
      {SECRET_KEYS.map(({ key, Icon, title }) => (
        <div key={key} className="flex items-center gap-2">
          <span title={title} className={status[key] ? "text-zinc-200" : "text-zinc-600"}>
            <Icon size={18} weight={status[key] ? "fill" : "regular"} />
          </span>
          <input
            type="password"
            value={values[key] ?? ""}
            placeholder={status[key] ? "••••••••" : title}
            onChange={(e) => setValues((v) => ({ ...v, [key]: e.target.value }))}
            onKeyDown={(e) => {
              if (e.key === "Enter") void handleSave(key);
            }}
            className="min-w-0 flex-1 bg-transparent text-sm text-zinc-100 outline-none placeholder:text-zinc-600"
          />
          {values[key] ? (
            <button
              type="button"
              title="Save"
              onClick={() => void handleSave(key)}
              className="rounded-md p-1 text-blue-400 hover:bg-zinc-800"
            >
              <FloppyDisk size={15} />
            </button>
          ) : null}
          {status[key] ? (
            <button
              type="button"
              title="Clear"
              onClick={() => void handleClear(key)}
              className="rounded-md p-1 text-zinc-600 hover:bg-zinc-800 hover:text-rose-400"
            >
              <Trash size={15} />
            </button>
          ) : null}
        </div>
      ))}
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
    <div className="space-y-2">
      {settings.fsExcludedPaths.map((path) => (
        <div key={path} className="flex items-center gap-2 text-sm text-zinc-300">
          <span className="min-w-0 flex-1 truncate">{path}</span>
          <button
            type="button"
            title="Remove"
            onClick={() => removePath(path)}
            className="rounded-md p-1 text-zinc-600 hover:bg-zinc-800 hover:text-rose-400"
          >
            <Trash size={14} />
          </button>
        </div>
      ))}
      <div className="flex items-center gap-2">
        <input
          type="text"
          value={newPath}
          placeholder="Documents/private"
          onChange={(e) => setNewPath(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && addPath()}
          className="min-w-0 flex-1 bg-transparent text-sm text-zinc-100 outline-none placeholder:text-zinc-600"
        />
        <button
          type="button"
          title="Add"
          onClick={addPath}
          className="rounded-md p-1 text-blue-400 hover:bg-zinc-800"
        >
          <Plus size={16} weight="bold" />
        </button>
      </div>
    </div>
  );
}

function ServicesSection() {
  const brainStatus = useAppStore((s) => s.brainStatus);
  const mlxStatus = useAppStore((s) => s.mlxStatus);
  const [busy, setBusy] = useState<"brain" | "mlx" | null>(null);

  async function handleBrain() {
    setBusy("brain");
    try {
      await restartBrain();
    } catch (err) {
      console.error(err);
    } finally {
      setBusy(null);
    }
  }

  async function handleMlx() {
    setBusy("mlx");
    try {
      await restartMlx();
    } catch (err) {
      console.error(err);
    } finally {
      setBusy(null);
    }
  }

  const brainOn = brainStatus === "online";
  const mlxOn = mlxStatus === "online";

  return (
    <div className="flex gap-2">
      <button
        type="button"
        title={busy === "brain" ? "Restarting Brain" : "Restart Brain"}
        onClick={() => void handleBrain()}
        disabled={busy !== null}
        className="flex h-12 flex-1 items-center justify-center rounded-xl border border-zinc-800 transition hover:border-zinc-700 disabled:opacity-40"
      >
        <Brain
          size={22}
          weight="duotone"
          className={
            busy === "brain"
              ? "animate-pulse text-amber-400"
              : brainOn
                ? "text-zinc-100"
                : "text-zinc-600"
          }
        />
      </button>
      <button
        type="button"
        title={busy === "mlx" ? "Restarting model" : "Restart model"}
        onClick={() => void handleMlx()}
        disabled={busy !== null}
        className="flex h-12 flex-1 items-center justify-center rounded-xl border border-zinc-800 transition hover:border-zinc-700 disabled:opacity-40"
      >
        <Cpu
          size={22}
          weight="duotone"
          className={
            busy === "mlx"
              ? "animate-pulse text-amber-400"
              : mlxOn
                ? "text-zinc-100"
                : "text-zinc-600"
          }
        />
      </button>
    </div>
  );
}

function CacheSection() {
  const [running, setRunning] = useState(false);

  async function handleRefresh() {
    setRunning(true);
    try {
      await refreshCache();
    } catch (err) {
      console.error(err);
    } finally {
      setRunning(false);
    }
  }

  return (
    <button
      type="button"
      title={running ? "Refreshing" : "Refresh cache"}
      onClick={() => void handleRefresh()}
      disabled={running}
      className="flex h-12 w-full items-center justify-center rounded-xl border border-zinc-800 text-zinc-400 transition hover:border-zinc-700 hover:text-zinc-200 disabled:opacity-40"
    >
      <ArrowsClockwise size={20} weight="bold" className={running ? "animate-spin" : ""} />
    </button>
  );
}
