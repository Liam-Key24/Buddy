import { FormEvent, useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Lightning } from "@phosphor-icons/react";
import { EmptyState } from "../components/ui/EmptyState";
import { ErrorBanner } from "../components/ui/ErrorBanner";
import { Button } from "../components/ui/Button";
import { Surface } from "../components/ui/Surface";
import { useToast } from "../components/ui/Toast";
import { useChatNav } from "../chatNav";
import {
  createSpark,
  dismissSpark,
  fetchSparks,
  promoteSpark,
  type ChatResponse,
  type Spark,
} from "../api";

export function SparksPage() {
  const [sparks, setSparks] = useState<Spark[]>([]);
  const [draft, setDraft] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [promotingId, setPromotingId] = useState<string | null>(null);
  const navigate = useNavigate();
  const { setConversationId } = useChatNav();
  const { pushToast } = useToast();

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
      pushToast("Spark saved");
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
      if (cid) setConversationId(cid);
      await reload();
      navigate("/chat");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not start a goal from this spark");
    } finally {
      setPromotingId(null);
    }
  }

  return (
    <section className="h-full overflow-y-auto p-5">
      <div className="mb-5">
        <p className="m-0 flex items-center gap-1.5 text-xs tracking-wide text-muted uppercase">
          <Lightning size={14} weight="duotone" />
          Sparks
        </p>
        <h1 className="mt-1 mb-0 font-display text-3xl font-medium">Ideas</h1>
      </div>

      {error ? <ErrorBanner className="mb-4">{error}</ErrorBanner> : null}

      <form
        className="mb-5 flex flex-col gap-2 rounded-float border border-hairline bg-raised-soft/70 p-3"
        onSubmit={onSubmit}
      >
        <label className="sr-only" htmlFor="spark-draft">
          Idea
        </label>
        <textarea
          id="spark-draft"
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          placeholder="A loose idea…"
          rows={2}
          className="w-full resize-none bg-transparent text-sm outline-none placeholder:text-muted-dim"
        />
        <div className="flex justify-end">
          <Button tone="primary" type="submit" disabled={busy || !draft.trim()}>
            Save
          </Button>
        </div>
      </form>

      {!sparks.length && (
        <EmptyState icon={<Lightning size={28} />}>No sparks yet. Capture one above.</EmptyState>
      )}

      <ul className="m-0 flex list-none flex-col gap-3 p-0">
        {sparks.map((s) => (
          <li key={s.id}>
            <Surface className="flex flex-wrap items-start justify-between gap-3">
              <div className="min-w-0 flex-1 text-sm">{s.content}</div>
              <div className="flex gap-2">
                <Button
                  tone="primary"
                  disabled={!!promotingId}
                  onClick={() => onPromote(s)}
                >
                  {promotingId === s.id ? "…" : "Turn into a goal"}
                </Button>
                <Button
                  tone="ghost"
                  disabled={!!promotingId}
                  onClick={async () => {
                    await dismissSpark(s.id);
                    pushToast("Spark dismissed");
                    await reload();
                  }}
                >
                  Dismiss
                </Button>
              </div>
            </Surface>
          </li>
        ))}
      </ul>
    </section>
  );
}
