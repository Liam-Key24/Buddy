import { Code } from "@phosphor-icons/react";
import { useCodeAgentStore } from "../stores/useCodeAgentStore";
import { StreamingMessageList } from "./StreamingMessageList";

export function CodeAgentWindow() {
  const { messages, streamingContent, isStreaming } = useCodeAgentStore();

  return (
    <StreamingMessageList
      messages={messages}
      streamingContent={streamingContent}
      isStreaming={isStreaming}
      showThinking={isStreaming && !streamingContent}
      empty={
        <div className="flex flex-1 flex-col items-center justify-center px-8 text-center text-zinc-500">
          <div className="mb-3 flex h-12 w-12 items-center justify-center rounded-2xl bg-violet-950 text-violet-400">
            <Code size={28} weight="duotone" />
          </div>
          <p className="max-w-sm text-sm">
            Code Agent runs Codex (GPT-5.5) in your selected workspace with your
            permissions. Pick a focus, attach files, and start a project.
          </p>
        </div>
      }
    />
  );
}
