import { useEffect, useRef } from "react";
import { Code } from "@phosphor-icons/react";
import { useCodeAgentStore } from "../stores/useCodeAgentStore";
import { MessageBubble } from "./MessageBubble";

export function CodeAgentWindow() {
  const { messages, streamingContent, isStreaming } = useCodeAgentStore();
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, streamingContent, isStreaming]);

  if (messages.length === 0 && !isStreaming) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center px-8 text-center text-zinc-500">
        <div className="mb-3 flex h-12 w-12 items-center justify-center rounded-2xl bg-violet-950 text-violet-400">
          <Code size={28} weight="duotone" />
        </div>
        <p className="max-w-sm text-sm">
          Code Agent runs Codex (GPT-5.5) in your selected workspace with your
          permissions. Pick a focus, attach files, and start a project.
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
        <MessageBubble role="assistant" content={streamingContent} />
      )}
      {isStreaming && !streamingContent && (
        <div className="mb-4 flex justify-start">
          <div className="rounded-2xl border border-zinc-800 bg-zinc-900 px-4 py-2.5 text-sm text-zinc-500 shadow-sm">
            Cursor is working...
          </div>
        </div>
      )}
      <div ref={bottomRef} />
    </div>
  );
}
