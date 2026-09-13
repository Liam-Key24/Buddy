import { FormEvent, useEffect, useState } from "react";
import {
  createSpark,
  dismissSpark,
  fetchSparks,
  promoteSpark,
  type Spark,
} from "../api";

export function SparksPage() {
  const [sparks, setSparks] = useState<Spark[]>([]);
  const [draft, setDraft] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function reload() {
    setSparks(await fetchSparks());
  }

  useEffect(() => {
    reload().catch((e: Error) => setError(e.message));
  }, []);

  async function onSubmit(e: FormEvent) {
    e.preventDefault();
    if (!draft.trim() || busy) return;
    setBusy(true);
    try {
      await createSpark(draft.trim());
      setDraft("");
      await reload();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed");
    } finally {
      setBusy(false);
    }
  }

  return (
    <section>
      <h1 className="page-title">Sparks</h1>
      <p className="page-sub">Capture ideas without turning them into commitments.</p>
      {error && <p className="muted">{error}</p>}

      <form className="composer" onSubmit={onSubmit}>
        <textarea
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          placeholder="A loose idea…"
          rows={2}
        />
        <button type="submit" disabled={busy || !draft.trim()}>
          Save spark
        </button>
      </form>

      <div className="panel">
        <h2>Open sparks</h2>
        {!sparks.length && <p className="muted">No sparks yet.</p>}
        <ul className="list">
          {sparks.map((s) => (
            <li key={s.id}>
              <div>{s.content}</div>
              <div className="spark-actions">
                <button
                  type="button"
                  className="primary"
                  onClick={async () => {
                    await promoteSpark(s.id);
                    await reload();
                  }}
                >
                  Promote to goal chat
                </button>
                <button
                  type="button"
                  className="ghost"
                  onClick={async () => {
                    await dismissSpark(s.id);
                    await reload();
                  }}
                >
                  Dismiss
                </button>
              </div>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}
