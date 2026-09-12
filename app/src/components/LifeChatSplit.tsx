import type { ReactNode } from "react";
import { ChatWindow } from "./ChatWindow";
import { ChatInput } from "./ChatInput";

export function LifeChatSplit({
  children,
  chatHeader,
  chat,
}: {
  children: ReactNode;
  chatHeader?: ReactNode;
  chat?: ReactNode;
}) {
  return (
    <div className="flex min-h-0 min-w-[40rem] flex-1 overflow-hidden">
      <div className="flex min-h-0 min-w-[18rem] flex-1 flex-col border-r border-zinc-800">
        {chatHeader}
        {chat ?? (
          <>
            <ChatWindow />
            <ChatInput />
          </>
        )}
      </div>
      <div className="flex min-h-0 w-[min(46%,36rem)] min-w-[24rem] max-w-[40rem] shrink-0 flex-col overflow-hidden">
        {children}
      </div>
    </div>
  );
}
