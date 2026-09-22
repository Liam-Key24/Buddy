import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
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
import { SectionHead } from "../components/ui/SectionHead";
import { Surface } from "../components/ui/Surface";
import { Tag } from "../components/ui/Tag";
import { API_BASE, APP_VERSION, fetchHealth, type HealthResponse } from "../api";

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
  const aiConfigured = health?.ai.configured === true;
  const aiEnabled = health?.ai.enabled !== false;

  return (
    <section className="h-full overflow-y-auto p-5">
      <div className="mb-5 flex flex-wrap items-end justify-between gap-3">
        <div>
          <p className="m-0 flex items-center gap-1.5 text-xs tracking-wide text-muted uppercase">
            <GearSix size={14} weight="duotone" />
            Settings
          </p>
          <h1 className="mt-1 mb-0 font-display text-3xl font-medium">About</h1>
          <p className="mt-1 mb-0 max-w-xl text-sm text-muted">
            Local-first planner — Chat needs Cloud AI; Today, Calendar, and Sparks work offline.
          </p>
        </div>
        <div className="flex items-center gap-2">
          <span className="text-xs text-muted">v{APP_VERSION}</span>
          {health ? (
            <Tag tone={health.ok ? "ok" : "danger"}>{health.ok ? "Healthy" : "Issue"}</Tag>
          ) : error ? (
            <Tag tone="danger">Offline</Tag>
          ) : (
            <Tag>Checking…</Tag>
          )}
        </div>
      </div>

      {error && (
        <div className="mb-4 rounded-card bg-danger/15 px-3 py-2 text-sm text-danger">
          Could not reach <code>{API_BASE}/health</code>: {error}
        </div>
      )}

      <div className="grid gap-3 lg:grid-cols-2">
        <Surface className="lg:col-span-2">
          <SectionHead
            icon={<Cloud size={16} weight="duotone" />}
            title="Cloud AI setup"
            action={
              health ? (
                <Tag tone={aiConfigured && aiEnabled ? "ok" : "warn"}>
                  {!aiEnabled ? "Disabled" : aiConfigured ? "Ready for Chat" : "Key needed"}
                </Tag>
              ) : null
            }
          />
          {!aiConfigured ? (
            <ol className="m-0 flex list-decimal flex-col gap-3 pl-5 text-sm">
              <li>
                <strong>Enable Zero Data Retention</strong>
                <p className="mt-1 mb-0 text-muted">
                  In the{" "}
                  <a href="https://console.groq.com" target="_blank" rel="noreferrer">
                    Groq console
                  </a>
                  , turn on Zero Data Retention for your org.
                </p>
              </li>
              <li>
                <strong>Create an API key</strong>
                <p className="mt-1 mb-0 text-muted">Buddy never shows the key in the UI.</p>
              </li>
              <li>
                <strong>Set the key</strong>
                <p className="mt-1 mb-0 text-muted">
                  Local: <code>GROQ_API_KEY=…</code> in <code>backend/.env</code>
                </p>
              </li>
              <li>
                <strong>Confirm</strong>
                <p className="mt-1 mb-0 text-muted">
                  Refresh — then open <Link to="/chat">Chat</Link>.
                </p>
              </li>
            </ol>
          ) : (
            <dl className="m-0 grid gap-2 text-sm">
              <div>
                <dt className="text-xs text-muted">Status</dt>
                <dd className="m-0">
                  Key set · {aiEnabled ? "enabled" : "disabled"}
                  {model ? (
                    <>
                      {" · "}
                      <code>{model}</code>
                    </>
                  ) : null}
                </dd>
              </div>
              <div>
                <dt className="text-xs text-muted">What needs AI</dt>
                <dd className="m-0">Chat planning and Sparks → goal. Today and Calendar work without it.</dd>
              </div>
            </dl>
          )}
        </Surface>

        <Surface>
          <SectionHead icon={<Info size={16} weight="duotone" />} title="Build" />
          <dl className="m-0 grid gap-2 text-sm">
            <div>
              <dt className="text-xs text-muted">Product</dt>
              <dd className="m-0">{product}</dd>
            </div>
            <div>
              <dt className="text-xs text-muted">Frontend</dt>
              <dd className="m-0">v{APP_VERSION}</dd>
            </div>
            <div>
              <dt className="text-xs text-muted">Privacy</dt>
              <dd className="m-0">{privacy}</dd>
            </div>
            <div>
              <dt className="text-xs text-muted">{aiLabel}</dt>
              <dd className="m-0">
                {model ? (
                  <>
                    <code>{model}</code>
                    {health && (
                      <span className="text-muted">
                        {" "}
                        · {health.ai.enabled ? "on" : "off"} ·{" "}
                        {health.ai.configured ? "key set" : "no key"}
                      </span>
                    )}
                  </>
                ) : (
                  <span className="text-muted">Waiting for backend</span>
                )}
              </dd>
            </div>
            <div>
              <dt className="text-xs text-muted">Database</dt>
              <dd className="m-0">
                {health?.db ? (
                  <code className="break-all">{health.db}</code>
                ) : (
                  <span className="text-muted">backend/data/buddy.db</span>
                )}
              </dd>
            </div>
          </dl>
        </Surface>

        <Surface>
          <SectionHead icon={<Terminal size={16} weight="duotone" />} title="Run locally" />
          <ol className="m-0 flex list-decimal flex-col gap-2 pl-5 text-sm text-muted">
            <li>
              Backend: <code>uvicorn app.main:app --host 127.0.0.1 --port 8787</code>
            </li>
            <li>
              Frontend: <code>npm run dev</code>
            </li>
            <li>
              Tauri: <code>cargo tauri dev</code>
            </li>
          </ol>
          <p className="mt-3 mb-0 flex items-center gap-1 text-xs text-muted">
            <BookOpen size={14} /> README.md
          </p>
        </Surface>

        <Surface className="lg:col-span-2">
          <SectionHead icon={<Code size={16} weight="duotone" />} title="Paths & keys" />
          <div className="grid gap-3 md:grid-cols-3">
            <div className="flex gap-2 text-sm">
              <MapPin size={18} weight="duotone" className="text-mint-dim" />
              <div>
                <strong>API</strong>
                <p className="m-0 text-muted">
                  <code>{API_BASE}</code>
                </p>
              </div>
            </div>
            <div className="flex gap-2 text-sm">
              <Database size={18} weight="duotone" className="text-mint-dim" />
              <div>
                <strong>SQLite</strong>
                <p className="m-0 text-muted">
                  <code>backend/data/buddy.db</code>
                </p>
              </div>
            </div>
            <div className="flex gap-2 text-sm">
              <Cloud size={18} weight="duotone" className="text-mint-dim" />
              <div>
                <strong>Groq</strong>
                <p className="m-0 text-muted">
                  <code>GROQ_API_KEY</code>
                </p>
              </div>
            </div>
          </div>
        </Surface>
      </div>
    </section>
  );
}
