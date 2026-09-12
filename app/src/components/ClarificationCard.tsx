import { useState } from "react";
import type { StructuredAsk } from "../stores/useChatStore";
import { FormattedText } from "./FormattedText";

const LETTERS = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";

interface Props {
  ask: StructuredAsk;
  disabled?: boolean;
  onResolve: (field: string, value: string) => void | Promise<void>;
}

export function ClarificationCard({ ask, disabled, onResolve }: Props) {
  const [selected, setSelected] = useState<string | null>(null);
  const [custom, setCustom] = useState("");
  const [submitting, setSubmitting] = useState(false);

  const isChoice = ask.ask_kind === "choice" && ask.options.length > 0;

  async function submit(value: string) {
    if (!value.trim() || submitting || disabled) return;
    setSubmitting(true);
    try {
      await onResolve(ask.field, value.trim());
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className="mb-4 max-w-lg rounded-2xl border border-zinc-800 bg-zinc-800 px-4 py-3">
      <div className="mb-3 text-sm text-zinc-200">
        <FormattedText text={ask.question} />
      </div>

      {isChoice && (
        <div className="mb-3 flex flex-col gap-2" role="radiogroup" aria-label="Choices">
          {ask.options.map((opt, i) => {
            const letter = LETTERS[i] ?? String(i + 1);
            const active = selected === opt.id;
            return (
              <button
                key={opt.id}
                type="button"
                role="radio"
                aria-checked={active}
                disabled={submitting || disabled}
                onClick={() => setSelected(opt.id)}
                className={
                  "flex items-start gap-3 rounded-xl border px-3 py-2 text-left text-sm transition " +
                  (active
                    ? "border-blue-500/60 bg-blue-950/40 text-zinc-100"
                    : "border-zinc-700 bg-zinc-950/50 text-zinc-300 hover:border-zinc-500")
                }
              >
                <span className="mt-0.5 flex h-6 w-6 shrink-0 items-center justify-center rounded-md border border-zinc-600 text-xs font-semibold text-zinc-400">
                  {letter}
                </span>
                <span>{opt.label}</span>
              </button>
            );
          })}
        </div>
      )}

      {!isChoice && (
        <input
          type="text"
          value={custom}
          onChange={(e) => setCustom(e.target.value)}
          disabled={submitting || disabled}
          placeholder="Type your answer…"
          className="mb-3 w-full rounded-xl border border-zinc-700 bg-zinc-950/60 px-3 py-2 text-sm text-zinc-200 outline-none focus:border-zinc-500"
          onKeyDown={(e) => {
            if (e.key === "Enter") void submit(custom);
          }}
        />
      )}

      <div className="flex justify-end gap-2">
        {isChoice ? (
          <button
            type="button"
            disabled={!selected || submitting || disabled}
            onClick={() => {
              const opt = ask.options.find((o) => o.id === selected);
              if (opt) void submit(opt.value);
            }}
            className="rounded-xl bg-blue-600 px-3 py-1.5 text-sm font-medium text-white disabled:opacity-40"
          >
            Confirm
          </button>
        ) : (
          <button
            type="button"
            disabled={!custom.trim() || submitting || disabled}
            onClick={() => void submit(custom)}
            className="rounded-xl bg-blue-600 px-3 py-1.5 text-sm font-medium text-white disabled:opacity-40"
          >
            Send
          </button>
        )}
      </div>
    </div>
  );
}
