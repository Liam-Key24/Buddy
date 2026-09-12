import { useEffect, useRef } from "react";

export function ThinkingTrace({
  steps,
}: {
  steps: { id: string; step: string; detail: string }[];
}) {
  const scrollerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = scrollerRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [steps]);

  if (steps.length === 0) {
    return (
      <div className="mb-3 flex items-center gap-2 px-0.5">
        <span className="thinking-pulse h-1.5 w-1.5 rounded-full bg-blue-400" />
        <span className="text-xs text-zinc-500">Thinking</span>
        <span className="thinking-dots flex items-center gap-1" aria-hidden>
          <span />
          <span />
          <span />
        </span>
      </div>
    );
  }

  return (
    <div className="mb-3 max-w-md">
      <div className="mb-1 flex items-center gap-2 px-0.5">
        <span className="thinking-pulse h-1.5 w-1.5 rounded-full bg-blue-400" />
        <span className="text-xs text-zinc-500">Thinking</span>
      </div>
      <div
        ref={scrollerRef}
        className="max-h-24 overflow-y-auto pr-1"
      >
        <ul className="space-y-1">
          {steps.map((s, i) => {
            const latest = i === steps.length - 1;
            return (
              <li
                key={s.id}
                className={
                  "text-xs leading-relaxed " +
                  (latest ? "text-zinc-400" : "text-zinc-600")
                }
              >
                <span>{s.step}</span>
                {s.detail ? (
                  <>
                    <span> · </span>
                    <span>{s.detail}</span>
                  </>
                ) : null}
              </li>
            );
          })}
        </ul>
      </div>
    </div>
  );
}
