import { useEffect, useRef } from "react";
import { ChatsCircle } from "@phosphor-icons/react";
import { useChatStore } from "../stores/useChatStore";
import { MessageBubble } from "./MessageBubble";

export function ChatWindow() {
  const { messages, streamingContent, isStreaming } = useChatStore();
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, streamingContent, isStreaming]);

  if (messages.length === 0 && !isStreaming) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center px-6 text-center">
        <div className="mb-4 flex h-14 w-14 items-center justify-center rounded-2xl bg-blue-950 text-blue-400">
          <ChatsCircle size={30} weight="duotone" />
        </div>
        <p className="text-base font-medium text-zinc-300">Start a conversation</p>
        <p className="mt-1 max-w-xs text-sm text-zinc-500">
          Ask anything, or format with bold, italic, and code from the toolbar.
        </p>
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-y-auto px-6 py-4">
      {messages.map((msg) => (
        <MessageBubble key={msg.id} role={msg.role} content={msg.content} />
      ))}
      {isStreaming && streamingContent && (
        <MessageBubble role="assistant" content={streamingContent} streaming />
      )}
      {isStreaming && !streamingContent && <ThinkingIndicator />}
      <div ref={bottomRef} />
    </div>
  );
}

function ThinkingIndicator() {
  return (
    <div className="mb-4 flex justify-start">
      <div className="thinking-bubble flex items-center gap-3 rounded-2xl border border-zinc-800 bg-zinc-900 px-4 py-3 shadow-sm">
        <span className="thinking-pulse flex h-2 w-2 rounded-full bg-blue-400" />
        <span className="text-sm text-zinc-400">Thinking</span>
        <span className="thinking-dots flex items-center gap-1" aria-hidden>
          <span />
          <span />
          <span />
        </span>
      </div>
    </div>
  );
}
