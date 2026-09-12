import { parseReplyBlocks } from "../lib/replyBlocks";

/** Structured chat surface: facts + lists, independent of surviving newlines. */
export function ReplyLayer({ text }: { text: string }) {
  const blocks = parseReplyBlocks(text);
  if (blocks.length === 0) return null;

  return (
    <div className="space-y-3">
      {blocks.map((block, i) => {
        if (block.type === "fact") {
          return (
            <div key={i}>
              <p className="text-[11px] font-medium uppercase tracking-wide text-zinc-500">
                {block.label}
              </p>
              {block.value ? (
                <p className="mt-0.5 text-sm text-zinc-100">{block.value}</p>
              ) : null}
            </div>
          );
        }
        if (block.type === "list") {
          return (
            <div key={i}>
              {block.title ? (
                <p className="text-[11px] font-medium uppercase tracking-wide text-zinc-500">
                  {block.title}
                </p>
              ) : null}
              <ul className={block.title ? "mt-1.5 space-y-1.5" : "space-y-1.5"}>
                {block.items.map((item, j) => (
                  <li
                    key={j}
                    className="rounded-lg border border-zinc-800 bg-zinc-950/40 px-2.5 py-1.5 text-sm text-zinc-200"
                  >
                    {item}
                  </li>
                ))}
              </ul>
            </div>
          );
        }
        return (
          <p key={i} className="whitespace-pre-wrap text-sm text-zinc-200">
            {stripMd(block.text)}
          </p>
        );
      })}
    </div>
  );
}

function stripMd(text: string): string {
  return text.replace(/\*\*(.+?)\*\*/g, "$1");
}
