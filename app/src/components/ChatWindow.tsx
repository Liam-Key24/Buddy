import { ChatsCircle } from "@phosphor-icons/react";
import { useChatStore } from "../stores/useChatStore";
import { ClarificationCard } from "./ClarificationCard";
import { StreamingMessageList } from "./StreamingMessageList";
import { resolveClarification } from "../lib/api";

export function ChatWindow() {
  const {
    messages,
    streamingContent,
    isStreaming,
    traceSteps,
    activeAsk,
    activeConversationId,
  } = useChatStore();

  const showThinking =
    isStreaming && (!streamingContent || traceSteps.length > 0);

  return (
    <StreamingMessageList
      messages={messages}
      streamingContent={streamingContent}
      isStreaming={isStreaming}
      showThinking={showThinking}
      traceSteps={traceSteps}
      empty={
        <div className="flex flex-1 flex-col items-center justify-center px-6 text-center">
          <div className="mb-4 flex h-14 w-14 items-center justify-center rounded-2xl bg-blue-950 text-blue-400">
            <ChatsCircle size={30} weight="duotone" />
          </div>
          <p className="text-base font-medium text-zinc-300">Start a conversation</p>
          <p className="mt-1 max-w-xs text-sm text-zinc-500">
            Ask anything, or format with bold, italic, and code from the toolbar.
          </p>
        </div>
      }
      footer={
        activeAsk && activeConversationId ? (
          <ClarificationCard
            ask={activeAsk}
            onResolve={async (field, value) => {
              useChatStore.getState().setActiveAsk(null);
              await resolveClarification(activeConversationId, field, value);
            }}
          />
        ) : null
      }
    />
  );
}
