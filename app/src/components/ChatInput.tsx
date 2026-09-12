import {
  useState,
  useRef,
  useEffect,
  useLayoutEffect,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import {
  PaperPlaneTilt,
  Square,
  TextB,
  TextItalic,
  Code,
  ListBullets,
  MagnifyingGlass,
} from "@phosphor-icons/react";
import { useChatStore } from "../stores/useChatStore";
import { useAppStore, type AppPage } from "../stores/useAppStore";
import { useConversationStore } from "../stores/useConversationStore";
import { sendMessage, createConversation, loadConversations, loadMessages, stopRun } from "../lib/api";
import { createResearchConversation, researchEnsure } from "../lib/lifeApi";

const MIN_HEIGHT = 40;
const MAX_HEIGHT = 200;

type FormatAction = "bold" | "italic" | "code" | "list";

export function ChatInput() {
  const [text, setText] = useState("");
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const { activeConversationId, isStreaming, setActiveConversationId, setMessages } =
    useChatStore();
  const conversations = useConversationStore((s) => s.conversations);
  const {
    pendingChatMessage,
    setPendingChatMessage,
    currentPage,
    setCurrentPage,
  } = useAppStore();
  const isResearchChat =
    conversations.find((c) => c.id === activeConversationId)?.kind === "research";

  useEffect(() => {
    if (pendingChatMessage) {
      setText(pendingChatMessage);
      setPendingChatMessage(null);
      requestAnimationFrame(() => textareaRef.current?.focus());
    }
  }, [pendingChatMessage, setPendingChatMessage]);

  useLayoutEffect(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = "auto";
    const next = Math.min(Math.max(el.scrollHeight, MIN_HEIGHT), MAX_HEIGHT);
    el.style.height = `${next}px`;
    el.style.overflowY = el.scrollHeight > MAX_HEIGHT ? "auto" : "hidden";
  }, [text]);

  async function handleSend() {
    const trimmed = text.trim();
    if (!trimmed || isStreaming) return;

    let convId = activeConversationId;
    const isNew = !convId;
    if (isNew) {
      const conv = await createConversation();
      convId = conv.id;
    }

    setText("");
    useChatStore.getState().beginSend(trimmed);
    if (isNew) {
      setActiveConversationId(convId);
    }

    try {
      await sendMessage(convId!, trimmed, { skipOptimistic: true });
    } catch (err) {
      console.error("send failed:", err);
    } finally {
      requestAnimationFrame(() => textareaRef.current?.focus());
    }
  }

  async function handleStop() {
    const store = useChatStore.getState();
    const id = store.activeConversationId;
    store.clearTrace();
    if (!id) {
      store.clearStreaming();
      return;
    }
    try {
      await stopRun(id);
    } catch (err) {
      console.error("stop failed:", err);
      useChatStore.getState().clearStreaming();
    }
  }

  async function startResearch() {
    if (isStreaming) return;
    if (isResearchChat) {
      requestAnimationFrame(() => textareaRef.current?.focus());
      return;
    }
    const conv = await createResearchConversation("Deep research");
    await loadConversations();
    await researchEnsure(conv.id, conv.title, text.trim()).catch(() => {});
    setActiveConversationId(conv.id);
    setMessages([]);
    await loadMessages(conv.id);
    setCurrentPage("chat");
    requestAnimationFrame(() => textareaRef.current?.focus());
  }

  function handleKeyDown(e: KeyboardEvent<HTMLTextAreaElement>) {
    if (e.key === "Escape" && isStreaming) {
      e.preventDefault();
      void handleStop();
      return;
    }
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
      return;
    }

    if ((e.metaKey || e.ctrlKey) && !e.shiftKey) {
      const key = e.key.toLowerCase();
      if (key === "b") {
        e.preventDefault();
        applyFormat("bold");
      } else if (key === "i") {
        e.preventDefault();
        applyFormat("italic");
      } else if (key === "e") {
        e.preventDefault();
        applyFormat("code");
      }
    }
  }

  function applyFormat(action: FormatAction) {
    const el = textareaRef.current;
    if (!el || isStreaming) return;

    const start = el.selectionStart;
    const end = el.selectionEnd;
    const selected = text.slice(start, end);
    let next = text;
    let cursorStart = start;
    let cursorEnd = end;

    if (action === "list") {
      if (selected) {
        const formatted = selected
          .split("\n")
          .map((line) => (line.startsWith("- ") ? line : `- ${line}`))
          .join("\n");
        next = text.slice(0, start) + formatted + text.slice(end);
        cursorStart = start;
        cursorEnd = start + formatted.length;
      } else {
        const lineStart = text.lastIndexOf("\n", start - 1) + 1;
        const lineEnd = text.indexOf("\n", start);
        const endIdx = lineEnd === -1 ? text.length : lineEnd;
        const line = text.slice(lineStart, endIdx);
        const formatted = line.startsWith("- ") ? line : `- ${line}`;
        next = text.slice(0, lineStart) + formatted + text.slice(endIdx);
        cursorStart = lineStart + formatted.length;
        cursorEnd = cursorStart;
      }
    } else {
      const wrappers: Record<Exclude<FormatAction, "list">, [string, string]> = {
        bold: ["**", "**"],
        italic: ["*", "*"],
        code: ["`", "`"],
      };
      const [open, close] = wrappers[action];
      const body = selected || "text";
      const insertion = `${open}${body}${close}`;
      next = text.slice(0, start) + insertion + text.slice(end);
      cursorStart = start + open.length;
      cursorEnd = cursorStart + body.length;
    }

    setText(next);
    requestAnimationFrame(() => {
      el.focus();
      el.setSelectionRange(cursorStart, cursorEnd);
    });
  }

  const canSend = Boolean(text.trim()) && !isStreaming;

  return (
    <div className="border-t border-zinc-800 px-4 py-3">
      <div
        className={`rounded-2xl border bg-zinc-800/50 transition-colors ${
          isStreaming
            ? "border-zinc-700"
            : "border-zinc-800 focus-within:border-zinc-600"
        }`}
      >
        <div className="flex items-center gap-0.5 border-b border-zinc-800/80 px-2 py-1.5">
          <FormatButton
            label="Bold"
            shortcut="⌘B"
            disabled={isStreaming}
            onClick={() => applyFormat("bold")}
          >
            <TextB size={15} weight="bold" />
          </FormatButton>
          <FormatButton
            label="Italic"
            shortcut="⌘I"
            disabled={isStreaming}
            onClick={() => applyFormat("italic")}
          >
            <TextItalic size={15} />
          </FormatButton>
          <FormatButton
            label="Code"
            shortcut="⌘E"
            disabled={isStreaming}
            onClick={() => applyFormat("code")}
          >
            <Code size={15} />
          </FormatButton>
          <FormatButton
            label="List"
            disabled={isStreaming}
            onClick={() => applyFormat("list")}
          >
            <ListBullets size={15} />
          </FormatButton>
          <span className="ml-auto hidden px-1 text-[11px] text-zinc-600 sm:inline">
            {isStreaming
              ? "Esc to stop"
              : "Enter to send · Shift+Enter for new line"}
          </span>
        </div>

        <div className="flex items-end gap-2 p-2">
          <textarea
            ref={textareaRef}
            value={text}
            onChange={(e) => setText(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={
              isStreaming
                ? "Waiting on the model…"
                : composerPlaceholder(currentPage, isResearchChat)
            }
            rows={1}
            className="chat-composer max-h-50 min-h-10 flex-1 resize-y bg-transparent px-2 py-2 text-sm leading-relaxed text-zinc-200 placeholder-zinc-500 outline-none"
            style={{ height: MIN_HEIGHT }}
          />
          {isStreaming ? (
            <button
              type="button"
              onClick={() => void handleStop()}
              aria-label="Stop run"
              title="Stop"
              className="mb-0.5 flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-red-500 text-white transition hover:bg-red-600"
            >
              <Square size={14} weight="fill" />
            </button>
          ) : (
            <button
              type="button"
              onClick={handleSend}
              disabled={!canSend}
              aria-label="Send message"
              className="mb-0.5 flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-blue-500 text-white transition hover:bg-blue-600 disabled:cursor-not-allowed disabled:opacity-40"
            >
              <PaperPlaneTilt size={16} weight="fill" />
            </button>
          )}
        </div>
      </div>
      <div className="mt-2 flex items-center justify-between gap-3 px-0.5">
        <div className="flex items-center gap-2">
          <button
            type="button"
            title="Deep research"
            aria-label="Deep research"
            aria-pressed={isResearchChat}
            disabled={isStreaming}
            onClick={() => void startResearch()}
            className={`inline-flex items-center gap-1 rounded-lg border px-2 py-1 text-[11px] font-medium transition disabled:opacity-40 ${
              isResearchChat
                ? "border-blue-500/40 bg-blue-500/15 text-blue-400"
                : "border-zinc-800 bg-zinc-800/60 text-zinc-500 hover:text-zinc-300"
            }`}
          >
            <MagnifyingGlass size={12} weight={isResearchChat ? "fill" : "regular"} />
            Research
          </button>
        </div>
        <p className="hidden text-[11px] text-zinc-600 sm:block">
          {isResearchChat
            ? "Deep research — local 14B, findings saved on this thread"
            : "Dump your day or ask anything — Buddy files it"}
        </p>
      </div>
    </div>
  );
}

function composerPlaceholder(page: AppPage, research: boolean): string {
  if (research) return "Dump a question to research…";
  switch (page) {
    case "documents":
      return "Paste a brief, or dump an idea into a doc…";
    case "calendar":
      return "Dump your day — pins, plans, what’s on…";
    default:
      return "Dump your day — Buddy files it…";
  }
}

function FormatButton({
  children,
  label,
  shortcut,
  disabled,
  onClick,
}: {
  children: ReactNode;
  label: string;
  shortcut?: string;
  disabled?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      title={shortcut ? `${label} (${shortcut})` : label}
      aria-label={label}
      disabled={disabled}
      onClick={onClick}
      onMouseDown={(e) => e.preventDefault()}
      className="flex h-7 w-7 items-center justify-center rounded-lg text-zinc-400 transition hover:bg-zinc-700/60 hover:text-zinc-200 disabled:opacity-40"
    >
      {children}
    </button>
  );
}
