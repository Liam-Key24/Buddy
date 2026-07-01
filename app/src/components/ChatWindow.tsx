import { useEffect, useRef } from "react";
import { useChatStore } from "../stores/useChatStore";
import { MessageBubble } from "./MessageBubble";

export function ChatWindow() {
  const { messages, streamingContent, isStreaming } = useChatStore();
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, streamingContent]);

  if (messages.length === 0 && !isStreaming) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center text-gray-500">
        <p className="text-lg font-medium">Welcome to Buddy</p>
        <p className="mt-1 text-sm">Start a conversation or try &quot;echo hello&quot;</p>
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
          <div className="rounded-2xl bg-gray-800 px-4 py-2.5 text-sm text-gray-400">
            Thinking...
          </div>
        </div>
      )}
      <div ref={bottomRef} />
    </div>
  );
}
