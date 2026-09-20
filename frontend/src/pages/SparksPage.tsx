import { FormEvent, useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Lightning } from "@phosphor-icons/react";
import { EmptyState } from "../components/EmptyState";
import { SectionHead } from "../components/SectionHead";
import {
  createSpark,
  dismissSpark,
  fetchSparks,
  promoteSpark,
  type ChatResponse,
  type Spark,
} from "../api";

const STORAGE_KEY = "buddy.conversationId";

export function SparksPage() {
  const [sparks, setSparks] = useState<Spark[]>([]);
  const [draft, setDraft] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [promotingId, setPromotingId] = useState<string | null>(null);
  const navigate = useNavigate();

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

  async function onPromote(s: Spark) {
    if (promotingId) return;
    setPromotingId(s.id);
    setError(null);
    try {
      const res = await promoteSpark(s.id);
      const chat = (res as { chat?: ChatResponse }).chat;
      const cid = chat?.conversation_id;
      if (cid) {
        localStorage.setItem(STORAGE_KEY, cid);
      }
      await reload();
      navigate("/chat");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not promote spark");
    } finally {
      setPromotingId(null);
    }
  }

  return (
    <section className="sparks-shell">
      <div className="sparks-panel sparks-hero">
        <div>
          <p className="page-kicker">
            <Lightning size={16} weight="duotone" />
            Sparks
          </p>
          <h1>Ideas</h1>
          <p className="muted">Capture without committing.</p>
        </div>
      </div>

      {error && <div className="error-banner">{error}</div>}

      <form className="composer frost-composer sparks-composer" onSubmit={onSubmit}>
        <textarea
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          placeholder="A loose idea…"
          rows={2}
        />
        <div className="composer-toolbar">
          <button className="btn primary" type="submit" disabled={busy || !draft.trim()}>
            Save
          </button>
        </div>
      </form>

      <div className="sparks-panel sparks-list-panel">
        <SectionHead
          title="Open"
          action={<span className="muted">{sparks.length}</span>}
        />
        {!sparks.length && <EmptyState>No sparks yet.</EmptyState>}
        <ul className="list sparks-list">
          {sparks.map((s) => (
            <li key={s.id} className="sparks-row">
              <div>{s.content}</div>
              <div className="actions">
                <button
                  type="button"
                  className="btn primary"
                  disabled={!!promotingId}
                  onClick={() => onPromote(s)}
                >
                  {promotingId === s.id ? "Promoting…" : "Promote"}
                </button>
                <button
                  type="button"
                  className="btn ghost"
                  disabled={!!promotingId}
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
