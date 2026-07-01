interface MessageBubbleProps {
  role: "user" | "assistant";
  content: string;
}

export function MessageBubble({ role, content }: MessageBubbleProps) {
  const isUser = role === "user";

  return (
    <div className={`flex ${isUser ? "justify-end" : "justify-start"} mb-4`}>
      <div
        className={`max-w-[75%] rounded-2xl px-4 py-2.5 text-sm leading-relaxed ${
          isUser
            ? "bg-blue-500 text-white shadow-sm"
            : "border border-zinc-800 bg-zinc-900 text-zinc-200 shadow-sm"
        }`}
      >
        {content}
      </div>
    </div>
  );
}
