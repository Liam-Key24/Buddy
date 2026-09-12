/** Turn a wall of prose into markdown the chat renderer can lay out. */

const LABEL =
  /^(Best free times|Free times|Found \d+ \w+|Proposed \d+ \w+|Next steps)\s*:\s*(.+)$/i;

const FACT =
  /^(Work|Sleep|Study|Gym|Focus|Climbing|Workout)\b(?:\s+is\s+on)?\s+(.+)$/i;

export function structureReply(text: string): string {
  const trimmed = text.trim();
  if (!trimmed || looksStructured(trimmed)) return text;

  return splitSentences(trimmed)
    .map((sentence) => formatSentence(sentence))
    .join("\n\n");
}

function looksStructured(text: string): boolean {
  if (/\n\s*(?:[-*•]|\d+\.)\s+/.test(text)) return true;
  if (/^#{1,3}\s+/m.test(text)) return true;
  if (/\n\n/.test(text)) return true;
  if (/\*\*[^*]+\*\*/.test(text) && text.includes("\n")) return true;
  return false;
}

function splitSentences(text: string): string[] {
  return text
    .split(/(?<=[.!?])\s+(?=[A-Z“"‘])/)
    .map((s) => s.trim())
    .filter(Boolean);
}

function formatSentence(sentence: string): string {
  const body = sentence.replace(/[.]+$/, "").trim();
  if (body.includes("\n")) return sentence;
  const labeled = LABEL.exec(body);
  if (labeled) {
    const title = labeled[1].replace(/^\w/, (c) => c.toUpperCase());
    const items = splitListItems(labeled[2]);
    if (items.length > 0) {
      return `**${title}**\n${items.map((item) => `- ${item}`).join("\n")}`;
    }
  }

  const fact = FACT.exec(body);
  if (fact) {
    return `**${capitalize(fact[1])}** — ${trimQuotes(fact[2])}`;
  }

  return sentence;
}

function splitListItems(rest: string): string[] {
  const quoted = splitQuoted(rest);
  if (quoted.length >= 1 && /[“"‘']/.test(rest)) return quoted;

  const ons = rest
    .split(/\s*(?:,|\band\b)\s*(?=on\b)/i)
    .map((part) => part.replace(/^and\s+/i, "").trim())
    .filter(Boolean);
  if (ons.length >= 2) return ons;

  return [];
}

function splitQuoted(rest: string): string[] {
  const items: string[] = [];
  const pattern =
    /[“"‘']([^”"'‘’]+)[”"'‘’](?:\s+on\s+(.+?))?(?=\s*,\s*[“"‘']|\s+and\s+[“"‘']|\s*$)/g;
  let match: RegExpExecArray | null;
  while ((match = pattern.exec(rest)) !== null) {
    const title = match[1].trim();
    const when = match[2]?.trim();
    items.push(when ? `${title} — ${when}` : title);
  }
  return items;
}

function trimQuotes(value: string): string {
  return value.replace(/^[“"‘']|[”"'‘’]$/g, "").trim();
}

function capitalize(value: string): string {
  return value.charAt(0).toUpperCase() + value.slice(1);
}
