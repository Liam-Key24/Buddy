import { Fragment, type ReactNode } from "react";
import { structureReply } from "../lib/structureReply";

/** Lightweight markdown: headings, lists, fenced code, links, bold/italic/code. */
export function FormattedText({ text }: { text: string }) {
  const blocks = parseBlocks(structureReply(text));
  if (blocks.length === 0) return null;

  return <div className="space-y-2.5">{blocks}</div>;
}

type FencePart =
  | { type: "text"; body: string }
  | { type: "code"; lang: string; body: string };

function splitFences(text: string): FencePart[] {
  const parts: FencePart[] = [];
  const pattern = /```(\w*)\n?([\s\S]*?)```/g;
  let last = 0;
  let match: RegExpExecArray | null;

  while ((match = pattern.exec(text)) !== null) {
    if (match.index > last) {
      parts.push({ type: "text", body: text.slice(last, match.index) });
    }
    parts.push({
      type: "code",
      lang: match[1] ?? "",
      body: (match[2] ?? "").replace(/\n$/, ""),
    });
    last = match.index + match[0].length;
  }

  if (last < text.length) {
    parts.push({ type: "text", body: text.slice(last) });
  }

  return parts;
}

function parseBlocks(text: string): ReactNode[] {
  const nodes: ReactNode[] = [];
  let key = 0;

  for (const part of splitFences(text)) {
    if (part.type === "code") {
      nodes.push(
        <pre
          key={key++}
          className="overflow-x-auto rounded-xl bg-zinc-950/60 px-3 py-2"
        >
          <code className="font-mono text-[0.85em] text-zinc-200">
            {part.body}
          </code>
        </pre>,
      );
      continue;
    }

    const lines = part.body.split("\n");
    let i = 0;
    while (i < lines.length) {
      const line = lines[i];
      if (!line.trim()) {
        i += 1;
        continue;
      }

      const heading = /^(#{1,3})\s+(.*)$/.exec(line);
      if (heading) {
        const level = heading[1].length;
        const cls =
          level === 1
            ? "text-base font-semibold text-zinc-100"
            : level === 2
              ? "text-sm font-semibold text-zinc-100"
              : "text-sm font-medium text-zinc-200";
        const Tag = (level === 1 ? "h3" : level === 2 ? "h4" : "h5") as
          | "h3"
          | "h4"
          | "h5";
        nodes.push(
          <Tag key={key++} className={cls}>
            {renderInline(heading[2])}
          </Tag>,
        );
        i += 1;
        continue;
      }

      if (/^[-*•]\s+/.test(line)) {
        const items: string[] = [];
        while (i < lines.length && /^[-*•]\s+/.test(lines[i])) {
          items.push(lines[i].replace(/^[-*•]\s+/, ""));
          i += 1;
        }
        nodes.push(
          <ul key={key++} className="list-disc space-y-1 pl-4 marker:text-zinc-600">
            {items.map((item, idx) => (
              <li key={idx}>{renderInline(item)}</li>
            ))}
          </ul>,
        );
        continue;
      }

      if (/^\d+\.\s+/.test(line)) {
        const items: string[] = [];
        while (i < lines.length && /^\d+\.\s+/.test(lines[i])) {
          items.push(lines[i].replace(/^\d+\.\s+/, ""));
          i += 1;
        }
        nodes.push(
          <ol key={key++} className="list-decimal space-y-1 pl-4 marker:text-zinc-600">
            {items.map((item, idx) => (
              <li key={idx}>{renderInline(item)}</li>
            ))}
          </ol>,
        );
        continue;
      }

      const para: string[] = [];
      while (
        i < lines.length &&
        lines[i].trim() &&
        !/^(#{1,3})\s+/.test(lines[i]) &&
        !/^[-*•]\s+/.test(lines[i]) &&
        !/^\d+\.\s+/.test(lines[i])
      ) {
        para.push(lines[i]);
        i += 1;
      }
      nodes.push(
        <p key={key++} className="wrap-break-word">
          {para.map((pLine, idx) => (
            <Fragment key={idx}>
              {idx > 0 && <br />}
              {renderInline(pLine)}
            </Fragment>
          ))}
        </p>,
      );
    }
  }

  return nodes;
}

function renderInline(text: string): ReactNode[] {
  const nodes: ReactNode[] = [];
  const pattern =
    /(\*\*[^*]+\*\*|\*[^*]+\*|`[^`]+`|\[[^\]]+\]\([^)]+\))/g;
  let last = 0;
  let match: RegExpExecArray | null;
  let key = 0;

  while ((match = pattern.exec(text)) !== null) {
    if (match.index > last) {
      nodes.push(text.slice(last, match.index));
    }
    const token = match[0];
    if (token.startsWith("**") && token.endsWith("**")) {
      nodes.push(
        <strong key={key++} className="font-semibold">
          {token.slice(2, -2)}
        </strong>,
      );
    } else if (token.startsWith("`") && token.endsWith("`")) {
      nodes.push(
        <code
          key={key++}
          className="rounded bg-black/25 px-1 py-0.5 font-mono text-[0.85em]"
        >
          {token.slice(1, -1)}
        </code>,
      );
    } else if (token.startsWith("*") && token.endsWith("*")) {
      nodes.push(
        <em key={key++} className="italic">
          {token.slice(1, -1)}
        </em>,
      );
    } else if (token.startsWith("[")) {
      const link = /^\[([^\]]+)\]\(([^)]+)\)$/.exec(token);
      if (link) {
        nodes.push(
          <a
            key={key++}
            href={link[2]}
            target="_blank"
            rel="noreferrer"
            className="text-blue-400 underline decoration-blue-400/40 underline-offset-2 hover:decoration-blue-400"
          >
            {link[1]}
          </a>,
        );
      } else {
        nodes.push(token);
      }
    } else {
      nodes.push(token);
    }
    last = match.index + token.length;
  }

  if (last < text.length) {
    nodes.push(text.slice(last));
  }

  return nodes;
}
