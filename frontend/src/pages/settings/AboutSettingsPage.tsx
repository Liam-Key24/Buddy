import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { BookOpen, Cloud, Code, Database, Info, MapPin, Terminal } from "@phosphor-icons/react";
import { SectionHead } from "../../components/ui/SectionHead";
import { Surface } from "../../components/ui/Surface";
import { Tag } from "../../components/ui/Tag";
import { API_BASE, APP_VERSION, fetchHealth, type HealthResponse } from "../../api";
import {
  SettingsDl,
  SettingsMasonry,
  SettingsMeta,
  SettingsPageHead,
} from "./settingsUi";
import { ErrorBanner } from "../../components/ui/ErrorBanner";

export function AboutSettingsPage() {
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
  const aiConfigured = health?.ai === "configured";
  const dbAvailable = health?.database === "available";
  const waiting = <SettingsMeta>Waiting for backend</SettingsMeta>;

  return (
    <>
      <SettingsPageHead
        title="About"
        subtitle="Local-first planner — Chat needs Cloud AI; Today, Calendar, and Sparks work offline."
        action={
          <div className="flex items-center gap-2">
            <SettingsMeta>v{APP_VERSION}</SettingsMeta>
            {health ? (
              <Tag tone={health.ok ? "ok" : "danger"}>{health.ok ? "Healthy" : "Issue"}</Tag>
            ) : error ? (
              <Tag tone="danger">Offline</Tag>
            ) : (
              <Tag>Checking…</Tag>
            )}
          </div>
        }
      />

      {error ? (
        <ErrorBanner>
          Could not reach <code>{API_BASE}/health</code>: {error}
        </ErrorBanner>
      ) : null}

      <SettingsMasonry>
        <Surface>
          <SectionHead
            icon={<Cloud size={16} weight="duotone" />}
            title="Cloud AI"
            action={
              health ? (
                <Tag tone={aiConfigured ? "ok" : "warn"}>
                  {aiConfigured ? "Ready" : "Key needed"}
                </Tag>
              ) : null
            }
          />
          {!aiConfigured ? (
            <ol className="settings-ol">
              <li>
                Enable Zero Data Retention in the{" "}
                <a href="https://console.groq.com" target="_blank" rel="noreferrer">
                  Groq console
                </a>
                .
              </li>
              <li>
                Set <code>GROQ_API_KEY</code> in <code>backend/.env</code>.
              </li>
              <li>
                Refresh, then open <Link to="/chat">Chat</Link>.
              </li>
            </ol>
          ) : (
            <SettingsDl
              rows={[
                { label: "Status", value: "Key set · ready for Chat and Sparks → goal" },
                {
                  label: "Offline without AI",
                  value: <SettingsMeta>Today and Calendar keep working locally.</SettingsMeta>,
                },
              ]}
            />
          )}
        </Surface>

        <Surface>
          <SectionHead icon={<Info size={16} weight="duotone" />} title="Build" />
          <SettingsDl
            rows={[
              { label: "Product", value: product },
              { label: "Frontend", value: `v${APP_VERSION}` },
              { label: "Privacy", value: privacy },
              {
                label: "Cloud AI",
                value: health ? (aiConfigured ? "configured" : "unconfigured") : waiting,
              },
              {
                label: "Database",
                value: health ? (dbAvailable ? "available" : "unavailable") : waiting,
              },
            ]}
          />
        </Surface>

        <Surface>
          <SectionHead icon={<Terminal size={16} weight="duotone" />} title="Run locally" />
          <ol className="settings-ol text-muted">
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
          <p className="mt-3 mb-0 flex items-center gap-1 settings-meta">
            <BookOpen size={14} /> README.md
          </p>
        </Surface>

        <Surface>
          <SectionHead icon={<Code size={16} weight="duotone" />} title="Paths" />
          <ul className="settings-list">
            {(
              [
                [MapPin, "API", <code key="api">{API_BASE}</code>],
                [Database, "SQLite", <code key="db">backend/data/buddy.db</code>],
                [Cloud, "Groq", <code key="groq">GROQ_API_KEY</code>],
              ] as const
            ).map(([Icon, label, value]) => (
              <li key={label} className="flex gap-2">
                <Icon size={18} weight="duotone" className="settings-path-icon" />
                <div>
                  <strong>{label}</strong>
                  <p className="m-0 break-all text-muted">{value}</p>
                </div>
              </li>
            ))}
          </ul>
        </Surface>
      </SettingsMasonry>
    </>
  );
}
