import { useState, KeyboardEvent, useEffect } from "react";
import { PaperPlaneTilt } from "@phosphor-icons/react";
import { useChatStore } from "../stores/useChatStore";
import { useAppStore } from "../stores/useAppStore";
import { sendMessage, createConversation } from "../lib/api";

export function ChatInput() {
  const [text, setText] = useState("");
  const { activeConversationId, isStreaming, setActiveConversationId } =
    useChatStore();
  const { pendingChatMessage, setPendingChatMessage } = useAppStore();

  useEffect(() => {
    if (pendingChatMessage) {
      setText(pendingChatMessage);
      setPendingChatMessage(null);
    }
  }, [pendingChatMessage, setPendingChatMessage]);

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
    }
  }

  function handleKeyDown(e: KeyboardEvent<HTMLTextAreaElement>) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  }

  return (
    <div className="border-t border-zinc-800 p-4">
      <div className="flex items-end gap-2 rounded-2xl border border-zinc-800 bg-zinc-800/50 p-2 shadow-sm">
        <textarea
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={handleKeyDown}
          disabled={isStreaming}
          placeholder="Message..."
          rows={1}
          className="flex-1 resize-none bg-transparent px-2 py-1.5 text-sm text-zinc-200 placeholder-zinc-500 outline-none disabled:opacity-50"
        />
        <button
          onClick={handleSend}
          disabled={!text.trim() || isStreaming}
          className="flex h-8 w-8 items-center justify-center rounded-xl bg-blue-500 text-white transition hover:bg-blue-600 disabled:cursor-not-allowed disabled:opacity-40"
        >
          <PaperPlaneTilt size={16} weight="fill" />
        </button>
      </div>
    </div>
  );
}
