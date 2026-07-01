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
      <div className="flex flex-1 flex-col items-center justify-center text-zinc-500">
        <div className="mb-3 flex h-12 w-12 items-center justify-center rounded-2xl bg-blue-950 text-blue-400">
          <ChatsCircle size={28} weight="duotone" />
        </div>
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-y-auto px-6 py-4">
      {messages.map((msg) => (
        <MessageBubble key={msg.id} role={msg.role} content={msg.content} />
      ))}
      {isStreaming && streamingContent && (
        <MessageBubble role="assistant" content={streamingContent} />
      )}
      {isStreaming && !streamingContent && (
        <div className="mb-4 flex justify-start">
          <div className="rounded-2xl border border-zinc-800 bg-zinc-900 px-4 py-2.5 text-sm text-zinc-500 shadow-sm">
            Thinking...
          </div>
        </div>
      )}
      <div ref={bottomRef} />
    </div>
  );
}
