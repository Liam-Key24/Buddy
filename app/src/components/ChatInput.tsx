import { useState, KeyboardEvent } from "react";
import { useChatStore } from "../stores/useChatStore";
import { sendMessage, createConversation } from "../lib/api";

export function ChatInput() {
  const [text, setText] = useState("");
  const { activeConversationId, isStreaming, setActiveConversationId } =
    useChatStore();

  async function handleSend() {
    const trimmed = text.trim();
    if (!trimmed || isStreaming) return;

    let convId = activeConversationId;
    if (!convId) {
      const conv = await createConversation();
      convId = conv.id;
      setActiveConversationId(conv.id);
    }

    setText("");
    try {
      await sendMessage(convId, trimmed);
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
    <div className="border-t border-gray-800 p-4">
      <div className="flex items-end gap-2 rounded-xl border border-gray-700 bg-gray-900/50 p-2">
        <textarea
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={handleKeyDown}
          disabled={isStreaming}
          placeholder="Message Buddy..."
          rows={1}
          className="flex-1 resize-none bg-transparent px-2 py-1.5 text-sm text-gray-100 placeholder-gray-500 outline-none disabled:opacity-50"
        />
        <button
          onClick={handleSend}
          disabled={!text.trim() || isStreaming}
          className="rounded-lg bg-indigo-600 px-4 py-1.5 text-sm font-medium text-white transition hover:bg-indigo-500 disabled:cursor-not-allowed disabled:opacity-40"
        >
          Send
        </button>
      </div>
    </div>
  );
}
