import { useEffect, useRef, type ReactNode } from "react";
import type { Message, TraceStep } from "../stores/useChatStore";
import { MessageBubble } from "./MessageBubble";
import { ThinkingTrace } from "./ThinkingTrace";

export function StreamingMessageList({
  messages,
  streamingContent,
  isStreaming,
  showThinking,
  traceSteps = [],
  empty,
  footer,
}: {
  messages: Pick<Message, "id" | "role" | "content">[];
  streamingContent: string;
  isStreaming: boolean;
  showThinking: boolean;
  traceSteps?: TraceStep[];
  empty: ReactNode;
  footer?: ReactNode;
}) {
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, streamingContent, isStreaming, traceSteps, footer]);

  if (messages.length === 0 && !isStreaming) {
    return <>{empty}</>;
  }

  return (
    <div className="flex-1 overflow-y-auto px-6 py-4">
      {messages.map((msg) => (
        <MessageBubble key={msg.id} role={msg.role} content={msg.content} />
      ))}
      {showThinking && <ThinkingTrace steps={traceSteps} />}
      {isStreaming && streamingContent && (
        <MessageBubble role="assistant" content={streamingContent} streaming />
      )}
      {footer}
      <div ref={bottomRef} />
    </div>
  );
}
