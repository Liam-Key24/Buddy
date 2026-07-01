import { useEffect, useState } from "react";
import { FloppyDisk } from "@phosphor-icons/react";
import { useSettingsStore } from "../stores/useSettingsStore";
import { loadSettings, saveSetting } from "../lib/api";

export function Settings() {
  const settings = useSettingsStore();
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    loadSettings().then((s) => {
      settings.setSettings({
        mlxUrl: s.mlx_url,
        brainUrl: s.brain_url,
        modelName: s.model_name,
        logLevel: s.log_level,
        autoStartMlx: s.auto_start_mlx,
      });
    });
  }, []);

  async function handleSave() {
    await saveSetting("model_name", settings.modelName);
    await saveSetting("log_level", settings.logLevel);
    await saveSetting("auto_start_mlx", String(settings.autoStartMlx));
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  }

  return (
    <div className="flex-1 overflow-y-auto p-6">
      <div className="mx-auto max-w-lg rounded-2xl border border-zinc-800 bg-zinc-900 p-6 shadow-sm">
        <div className="space-y-4">
          <Field label="MLX URL" value={settings.mlxUrl} readOnly />
          <Field label="Brain URL" value={settings.brainUrl} readOnly />
          <Field
            label="Model"
            value={settings.modelName}
            onChange={(v) => settings.setSettings({ modelName: v })}
          />
          <div>
            <label className="mb-1 block text-xs font-medium text-zinc-400">
              Log Level
            </label>
            <select
              value={settings.logLevel}
              onChange={(e) => settings.setSettings({ logLevel: e.target.value })}
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

          <button
            onClick={handleSave}
            className="flex items-center gap-2 rounded-xl bg-blue-500 px-4 py-2 text-sm font-medium text-white transition hover:bg-blue-600"
          >
            <FloppyDisk size={16} weight="duotone" />
            {saved ? "Saved" : "Save"}
          </button>
        </div>
      </div>
    </div>
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
