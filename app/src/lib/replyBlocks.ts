export type ReplyBlock =
  | { type: "text"; text: string }
  | { type: "fact"; label: string; value: string }
  | { type: "list"; title?: string; items: string[] };

/** Universal parse: works with real markdown or a flattened one-line dump. */
export function parseReplyBlocks(raw: string): ReplyBlock[] {
  const text = raw.replace(/\r\n/g, "\n").trim();
  if (!text) return [];
  return parseLined(restoreStructure(text));
}

export function restoreStructure(text: string): string {
  if (/\n\s*[-*•]\s+/.test(text) || /\n\n/.test(text)) return text;
  return text
    .replace(/\s+\*\*/g, "\n\n**")
    .replace(/\s+-\s+(?=[A-Z0-9“"‘])/g, "\n- ")
    .trim();
}

function parseLined(text: string): ReplyBlock[] {
  const lines = text.split("\n");
  const blocks: ReplyBlock[] = [];
  let i = 0;

  while (i < lines.length) {
    const line = lines[i];
    if (!line.trim()) {
      i += 1;
      continue;
    }

    if (/^[-*•]\s+/.test(line)) {
      const items: string[] = [];
      while (i < lines.length && /^[-*•]\s+/.test(lines[i])) {
        items.push(lines[i].replace(/^[-*•]\s+/, "").trim());
        i += 1;
      }
      const prev = blocks[blocks.length - 1];
      if (prev?.type === "fact" && !prev.value) {
        blocks[blocks.length - 1] = { type: "list", title: prev.label, items };
      } else {
        blocks.push({ type: "list", items });
      }
      continue;
    }

    const fact = /^\*\*(.+?)\*\*(?:\s*—\s*(.*))?$/.exec(line.trim());
    if (fact) {
      blocks.push({ type: "fact", label: fact[1].trim(), value: (fact[2] ?? "").trim() });
      i += 1;
      continue;
    }

    const buf: string[] = [];
    while (
      i < lines.length &&
      lines[i].trim() &&
      !/^[-*•]\s+/.test(lines[i]) &&
      !/^\*\*(.+?)\*\*/.test(lines[i].trim())
    ) {
      buf.push(lines[i]);
      i += 1;
    }
    if (buf.length) blocks.push({ type: "text", text: buf.join("\n") });
  }

  return blocks;
}
