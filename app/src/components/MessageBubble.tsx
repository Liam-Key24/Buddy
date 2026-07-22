import { FormattedText } from "./FormattedText";

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
        className={`max-w-[75%] rounded-2xl px-4 py-2.5 text-sm leading-relaxed ${
          isUser
            ? "bg-blue-500 text-white shadow-sm"
            : "border border-zinc-800 bg-zinc-900 text-zinc-200 shadow-sm"
        }`}
      >
        <FormattedText text={content} />
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
