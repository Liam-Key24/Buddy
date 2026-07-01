import { useEffect, useState } from "react";
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
      <h2 className="mb-6 text-lg font-semibold text-gray-200">Settings</h2>

      <div className="max-w-lg space-y-4">
        <Field label="MLX URL" value={settings.mlxUrl} readOnly />
        <Field label="Brain URL" value={settings.brainUrl} readOnly />
        <Field
          label="Model"
          value={settings.modelName}
          onChange={(v) => settings.setSettings({ modelName: v })}
        />
        <div>
          <label className="mb-1 block text-xs font-medium text-gray-400">
            Log Level
          </label>
          <select
            value={settings.logLevel}
            onChange={(e) => settings.setSettings({ logLevel: e.target.value })}
            className="w-full rounded-lg border border-gray-700 bg-gray-900 px-3 py-2 text-sm text-gray-200 outline-none focus:border-indigo-500"
          >
            <option value="debug">debug</option>
            <option value="info">info</option>
            <option value="warn">warn</option>
            <option value="error">error</option>
          </select>
        </div>
        <label className="flex items-center gap-2 text-sm text-gray-300">
          <input
            type="checkbox"
            checked={settings.autoStartMlx}
            onChange={(e) =>
              settings.setSettings({ autoStartMlx: e.target.checked })
            }
            className="rounded border-gray-600"
          />
          Auto-start MLX server
        </label>

        <button
          onClick={handleSave}
          className="rounded-lg bg-indigo-600 px-4 py-2 text-sm font-medium text-white transition hover:bg-indigo-500"
        >
          {saved ? "Saved!" : "Save settings"}
        </button>
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
      <label className="mb-1 block text-xs font-medium text-gray-400">
        {label}
      </label>
      <input
        type="text"
        value={value}
        readOnly={readOnly}
        onChange={(e) => onChange?.(e.target.value)}
        className="w-full rounded-lg border border-gray-700 bg-gray-900 px-3 py-2 text-sm text-gray-200 outline-none focus:border-indigo-500 read-only:opacity-60"
      />
    </div>
  );
}
