import { useEffect, useRef } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import {
  subscribeTerminal,
  terminalClose,
  terminalOpen,
  terminalResize,
  terminalWrite,
} from "../lib/api";

export function CodeTerminal({ cwd }: { cwd?: string | null }) {
  const containerRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const idRef = useRef<string | null>(null);

  useEffect(() => {
    if (!containerRef.current) return;

    const term = new Terminal({
      fontSize: 12,
      fontFamily:
        "ui-monospace, SFMono-Regular, Menlo, Monaco, monospace",
      theme: { background: "#09090b", foreground: "#e4e4e7" },
      cursorBlink: true,
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(containerRef.current);
    fit.fit();
    termRef.current = term;

    let disposed = false;
    let unsub: (() => void) | null = null;

    (async () => {
      const id = await terminalOpen(cwd ?? undefined, term.cols, term.rows);
      if (disposed) {
        terminalClose(id).catch(() => {});
        return;
      }
      idRef.current = id;

      unsub = subscribeTerminal(
        (outId, data) => {
          if (outId === id) term.write(data);
        },
        (exitId) => {
          if (exitId === id) term.write("\r\n[process exited]\r\n");
        },
      );

      term.onData((data) => {
        terminalWrite(id, data).catch(() => {});
      });
    })();

    const resizeObserver = new ResizeObserver(() => {
      try {
        fit.fit();
        if (idRef.current) {
          terminalResize(idRef.current, term.cols, term.rows).catch(() => {});
        }
      } catch {
        /* ignore */
      }
    });
    resizeObserver.observe(containerRef.current);

    return () => {
      disposed = true;
      resizeObserver.disconnect();
      unsub?.();
      if (idRef.current) terminalClose(idRef.current).catch(() => {});
      term.dispose();
      termRef.current = null;
      idRef.current = null;
    };
    // Re-open the terminal when the workspace changes.
  }, [cwd]);

  return <div ref={containerRef} className="h-full w-full bg-zinc-950 p-1" />;
}
