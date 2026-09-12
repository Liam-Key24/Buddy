import { useEffect } from "react";
import { subscribeLifeEvent } from "../lib/lifeApi";
import { useLifeChatContext } from "../stores/useLifeChatContext";

export function useLifePage({
  event,
  load,
  loadDeps = [],
  context,
}: {
  event?: string;
  load?: () => void | Promise<void>;
  loadDeps?: unknown[];
  context: string;
}) {
  const setContext = useLifeChatContext((s) => s.setContext);
  const clearContext = useLifeChatContext((s) => s.clearContext);

  useEffect(() => {
    if (!load) return;
    Promise.resolve(load()).catch(console.error);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- match prior page mount/reload deps
  }, loadDeps);

  useEffect(() => {
    if (!event || !load) return;
    return subscribeLifeEvent(event, () => {
      Promise.resolve(load()).catch(console.error);
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [event, ...loadDeps]);

  useEffect(() => {
    setContext(context);
    return () => clearContext();
  }, [context, setContext, clearContext]);
}
