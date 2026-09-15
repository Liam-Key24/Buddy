import { useEffect, useState } from "react";
import {
  BookOpen,
  Cloud,
  Code,
  Database,
  GearSix,
  Info,
  MapPin,
  Terminal,
} from "@phosphor-icons/react";
import { SectionHead } from "../components/SectionHead";
import { API_BASE, APP_VERSION, fetchHealth, type HealthResponse } from "../api";

function StatusPill({ ok, label }: { ok: boolean; label: string }) {
  return <span className={`settings-pill${ok ? " ok" : ""}`}>{label}</span>;
}

export function SettingsPage() {
  const [health, setHealth] = useState<HealthResponse | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    fetchHealth()
      .then((row) => {
        if (!cancelled) setHealth(row);
      })
      .catch((e: Error) => {
        if (!cancelled) setError(e.message);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const product = health?.product ?? "Buddy";
  const privacy = health?.privacy ?? "Local data · Cloud reasoning";
  const model = health?.ai.model_configured;
  const aiLabel = health?.ai.label ?? "Cloud AI";

  return (
    <section className="settings-shell">
      <div className="settings-panel settings-hero">
        <div>
          <p className="page-kicker">
            <GearSix size={16} weight="duotone" />
            Settings
          </p>
          <h1 className="today-date">About</h1>
          <p className="today-sub">
            Local-dev reference — Cloud AI is set via backend <code>.env</code> or Keychain.
          </p>
        </div>
        <div className="settings-hero-meta">
          <span className="settings-version">v{APP_VERSION}</span>
          {health ? (
            <StatusPill ok={health.ok} label={health.ok ? "Healthy" : "Issue"} />
          ) : error ? (
            <StatusPill ok={false} label="Offline" />
          ) : (
            <StatusPill ok={false} label="Checking…" />
          )}
        </div>
      </div>

      {error && (
        <div className="error-banner today-error">
          Could not reach <code>{API_BASE}/health</code>: {error}
        </div>
      )}

      <div className="settings-grid">
        <div className="settings-panel">
          <SectionHead
            title={
              <>
                <Info size={18} weight="duotone" />
                Build
              </>
            }
          />
          <dl className="settings-dl">
            <div>
              <dt>Product</dt>
              <dd>{product}</dd>
            </div>
            <div>
              <dt>Frontend</dt>
              <dd>v{APP_VERSION}</dd>
            </div>
            <div>
              <dt>Privacy</dt>
              <dd>{privacy}</dd>
            </div>
            <div>
              <dt>{aiLabel}</dt>
              <dd>
                {model ? (
                  <>
                    <code>{model}</code>
                    {health && (
                      <span className="muted settings-inline-meta">
                        {" "}
                        · {health.ai.enabled ? "on" : "off"}
                        {" · "}
                        {health.ai.configured ? "key set" : "no key"}
                      </span>
                    )}
                  </>
                ) : (
                  <span className="muted">Waiting for backend</span>
                )}
              </dd>
            </div>
            <div>
              <dt>Database</dt>
              <dd>
                {health?.db ? (
                  <code className="settings-path">{health.db}</code>
                ) : (
                  <span className="muted">backend/data/buddy.db</span>
                )}
              </dd>
            </div>
            {health?.host && (
              <div>
                <dt>Host</dt>
                <dd>
                  <code>{health.host}</code>
                </dd>
              </div>
            )}
          </dl>
        </div>

        <div className="settings-panel">
          <SectionHead
            title={
              <>
                <Terminal size={18} weight="duotone" />
                Run locally
              </>
            }
          />
          <ol className="settings-steps">
            <li>
              <strong>Backend</strong>
              <p className="muted">
                <code>cd backend</code> → <code>.venv</code> →{" "}
                <code>pip install -r requirements.txt</code> →{" "}
                <code>PYTHONPATH=. uvicorn app.main:app --host 127.0.0.1 --port 8787 --reload</code>
              </p>
            </li>
            <li>
              <strong>Frontend</strong>
              <p className="muted">
                <code>cd frontend</code> → <code>npm install</code> → <code>npm run dev</code>
              </p>
            </li>
            <li>
              <strong>Tauri</strong>
              <p className="muted">
                <code>cargo tauri dev</code> from <code>src-tauri</code> — see{" "}
                <code>docs/PACKAGING.md</code>.
              </p>
            </li>
          </ol>
          <p className="settings-docs muted">
            <BookOpen size={14} weight="duotone" /> <code>README.md</code>
          </p>
        </div>

        <div className="settings-panel settings-span">
          <SectionHead
            title={
              <>
                <Code size={18} weight="duotone" />
                Paths & keys
              </>
            }
          />
          <div className="settings-info-grid">
            <div className="settings-info-card">
              <MapPin size={18} weight="duotone" />
              <div>
                <strong>API</strong>
                <p className="muted">
                  <code>{API_BASE}</code>
                  {import.meta.env.VITE_API_BASE
                    ? " (VITE_API_BASE)"
                    : " → :8787"}
                </p>
              </div>
            </div>
            <div className="settings-info-card">
              <Database size={18} weight="duotone" />
              <div>
                <strong>SQLite</strong>
                <p className="muted">
                  <code>backend/data/buddy.db</code> · packaged builds use{" "}
                  <code>BUDDY_DB_PATH</code>
                </p>
              </div>
            </div>
            <div className="settings-info-card">
              <Cloud size={18} weight="duotone" />
              <div>
                <strong>Groq</strong>
                <p className="muted">
                  <code>GROQ_API_KEY</code> in <code>backend/.env</code> (never commit). Optional:{" "}
                  <code>GROQ_MODEL</code>, <code>BUDDY_AI_ENABLED</code>.
                </p>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
