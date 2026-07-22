import { useState, KeyboardEvent } from "react";
import { CaretDown, Paperclip, PaperPlaneTilt, X } from "@phosphor-icons/react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  FocusMode,
  FOCUS_LABELS,
  useCodeAgentStore,
} from "../stores/useCodeAgentStore";
import {
  createCodexConversation,
  sendCodexMessage,
  setConversationFocus,
} from "../lib/api";
import { useConversationStore } from "../stores/useConversationStore";

const FOCUS_ORDER: FocusMode[] = [
  "planning",
  "asking",
  "debugging",
  "focused",
];

function basename(path: string) {
  return path.split("/").pop() || path;
}

export function CodeAgentInput() {
  const [text, setText] = useState("");
  const [focusOpen, setFocusOpen] = useState(false);
  const {
    activeConversationId,
    isStreaming,
    focus,
    attachments,
    setActiveConversationId,
    setFocus,
    addAttachment,
    removeAttachment,
    clearAttachments,
    beginSend,
  } = useCodeAgentStore();

  async function chooseFocus(mode: FocusMode) {
    setFocus(mode);
    setFocusOpen(false);
    if (activeConversationId) {
      try {
        await setConversationFocus(activeConversationId, mode);
      } catch (err) {
        console.error("set focus failed:", err);
      }
    }
  }

  async function handleAttach() {
    try {
      const selected = await open({ multiple: true, directory: false });
      if (!selected) return;
      const paths = Array.isArray(selected) ? selected : [selected];
      paths.forEach((p) => addAttachment(p));
    } catch (err) {
      console.error("attach failed:", err);
    }
  }

  async function handleSend() {
    const trimmed = text.trim();
    if (!trimmed || isStreaming) return;

    let convId = activeConversationId;
    if (!convId) {
      const conv = await createCodexConversation("New project", focus);
      convId = conv.id;
      setActiveConversationId(convId);
      useConversationStore.getState().addConversation(conv);
    }

    const sentAttachments = [...attachments];
    setText("");
    clearAttachments();
    beginSend(trimmed);

    try {
      // #region agent log
      fetch('http://127.0.0.1:7853/ingest/712ccc2a-29d1-4885-8261-d223e160088e',{method:'POST',headers:{'Content-Type':'application/json','X-Debug-Session-Id':'4e7020'},body:JSON.stringify({sessionId:'4e7020',location:'CodeAgentInput.tsx:handleSend',message:'Sending message to backend',data:{convId},timestamp:Date.now(),runId:'run1',hypothesisId:'Backend Error Early Exit'})}).catch(()=>{});
      // #endregion
      await sendCodexMessage(convId, trimmed, focus, sentAttachments);
      // #region agent log
      fetch('http://127.0.0.1:7853/ingest/712ccc2a-29d1-4885-8261-d223e160088e',{method:'POST',headers:{'Content-Type':'application/json','X-Debug-Session-Id':'4e7020'},body:JSON.stringify({sessionId:'4e7020',location:'CodeAgentInput.tsx:handleSend',message:'Message sent successfully',data:{},timestamp:Date.now(),runId:'run1',hypothesisId:'Backend Error Early Exit'})}).catch(()=>{});
      // #endregion
    } catch (err) {
      console.error("codex send failed:", err);
      // #region agent log
      fetch('http://127.0.0.1:7853/ingest/712ccc2a-29d1-4885-8261-d223e160088e',{method:'POST',headers:{'Content-Type':'application/json','X-Debug-Session-Id':'4e7020'},body:JSON.stringify({sessionId:'4e7020',location:'CodeAgentInput.tsx:handleSend',message:'Error caught in frontend',data:{error:String(err)},timestamp:Date.now(),runId:'run1',hypothesisId:'Backend Error Early Exit'})}).catch(()=>{});
      // #endregion
    }
  }

  function handleKeyDown(e: KeyboardEvent<HTMLTextAreaElement>) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  }

  return (
    <div className="border-t border-zinc-800 p-4">
      <div className="flex flex-col gap-2 rounded-2xl border border-zinc-800 bg-zinc-800/50 p-2 shadow-sm">
        <div className="flex items-center gap-2">
          <div className="relative">
            <button
              type="button"
              onClick={() => setFocusOpen((o) => !o)}
              className="flex items-center gap-1 rounded-lg bg-zinc-800 px-2.5 py-1 text-xs font-medium text-zinc-300 transition hover:bg-zinc-700"
            >
              {FOCUS_LABELS[focus]}
              <CaretDown size={12} weight="bold" />
            </button>
            {focusOpen && (
              <div className="absolute bottom-full left-0 mb-1 w-36 overflow-hidden rounded-xl border border-zinc-700 bg-zinc-900 shadow-lg">
                {FOCUS_ORDER.map((mode) => (
                  <button
                    key={mode}
                    type="button"
                    onClick={() => chooseFocus(mode)}
                    className={`block w-full px-3 py-2 text-left text-xs transition hover:bg-zinc-800 ${
                      mode === focus ? "text-violet-400" : "text-zinc-300"
                    }`}
                  >
                    {FOCUS_LABELS[mode]}
                  </button>
                ))}
              </div>
            )}
          </div>
          <button
            type="button"
            onClick={handleAttach}
            title="Attach file"
            className="flex items-center gap-1 rounded-lg bg-zinc-800 px-2.5 py-1 text-xs font-medium text-zinc-300 transition hover:bg-zinc-700"
          >
            <Paperclip size={13} weight="bold" />
            Attach
          </button>
        </div>

        {attachments.length > 0 && (
          <div className="flex flex-wrap gap-1.5 px-1">
            {attachments.map((path) => (
              <span
                key={path}
                className="flex items-center gap-1 rounded-md bg-zinc-800 px-2 py-1 text-[11px] text-zinc-300"
                title={path}
              >
                {basename(path)}
                <button
                  type="button"
                  onClick={() => removeAttachment(path)}
                  className="text-zinc-500 transition hover:text-rose-400"
                >
                  <X size={11} weight="bold" />
                </button>
              </span>
            ))}
          </div>
        )}

        <div className="flex items-end gap-2">
          <textarea
            value={text}
            onChange={(e) => setText(e.target.value)}
            onKeyDown={handleKeyDown}
            disabled={isStreaming}
            placeholder="Ask, plan, or build with Codex..."
            rows={1}
            className="flex-1 resize-none bg-transparent px-2 py-1.5 text-sm text-zinc-200 placeholder-zinc-500 outline-none disabled:opacity-50"
          />
          <button
            onClick={handleSend}
            disabled={!text.trim() || isStreaming}
            className="flex h-8 w-8 items-center justify-center rounded-xl bg-violet-500 text-white transition hover:bg-violet-600 disabled:cursor-not-allowed disabled:opacity-40"
          >
            <PaperPlaneTilt size={16} weight="fill" />
          </button>
        </div>
      </div>
    </div>
  );
}
