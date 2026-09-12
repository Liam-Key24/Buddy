import { FormattedText } from "./FormattedText";
import { ReplyLayer } from "./ReplyLayer";

interface MessageBubbleProps {
  role: "user" | "assistant";
  content: string;
  streaming?: boolean;
}

export function MessageBubble({
  role,
  content,
  streaming = false,
}: MessageBubbleProps) {
  const isUser = role === "user";

  return (
    <div className={`mb-4 flex ${isUser ? "justify-end" : "justify-start"}`}>
      <div
        className={`rounded-2xl text-sm leading-relaxed ${
          isUser
            ? "max-w-[75%] bg-blue-500 px-4 py-2.5 text-white"
            : "max-w-[min(42rem,92%)] border border-zinc-800 bg-zinc-900 px-4 py-3 text-zinc-200"
        }`}
      >
        {isUser ? <FormattedText text={content} /> : <ReplyLayer text={content} />}
        {streaming && (
          <span
            className="streaming-caret ml-0.5 inline-block h-3.5 w-0.5 translate-y-0.5 bg-current align-middle"
            aria-hidden
          />
        )}
      </div>
    </div>
  );
}
