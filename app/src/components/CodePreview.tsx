import { useEffect, useState } from "react";
import { ArrowClockwise, Globe } from "@phosphor-icons/react";
import { useCodeAgentStore } from "../stores/useCodeAgentStore";

export function CodePreview() {
  const { previewUrl, setPreviewUrl } = useCodeAgentStore();
  const [urlInput, setUrlInput] = useState("");
  const [iframeKey, setIframeKey] = useState(0);

  useEffect(() => {
    if (previewUrl) setUrlInput(previewUrl);
  }, [previewUrl]);

  function load() {
    const trimmed = urlInput.trim();
    if (!trimmed) return;
    const normalized = /^https?:\/\//.test(trimmed)
      ? trimmed
      : `http://${trimmed}`;
    setPreviewUrl(normalized);
    setIframeKey((k) => k + 1);
  }

  return (
    <div className="flex h-full flex-col bg-zinc-950">
      <div className="flex items-center gap-1.5 border-b border-zinc-800 px-2 py-1.5">
        <Globe size={14} className="shrink-0 text-zinc-500" />
        <input
          type="text"
          value={urlInput}
          placeholder="localhost:5173"
          onChange={(e) => setUrlInput(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && load()}
          className="min-w-0 flex-1 rounded-md bg-zinc-900 px-2 py-1 text-xs text-zinc-200 outline-none focus:ring-1 focus:ring-violet-500/40"
        />
        <button
          onClick={() => setIframeKey((k) => k + 1)}
          title="Reload"
          className="shrink-0 rounded-md p-1 text-zinc-500 transition hover:bg-zinc-800 hover:text-zinc-300"
        >
          <ArrowClockwise size={14} />
        </button>
      </div>
      <div className="flex-1">
        {previewUrl ? (
          <iframe
            key={iframeKey}
            src={previewUrl}
            title="preview"
            className="h-full w-full border-0 bg-white"
          />
        ) : (
          <div className="flex h-full items-center justify-center px-4 text-center text-xs text-zinc-600">
            When the agent starts a dev server, its URL appears here
            automatically. You can also type one above.
          </div>
        )}
      </div>
    </div>
  );
}
