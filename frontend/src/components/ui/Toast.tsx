import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { cn } from "../../lib/cn";

type ToastAction = { label: string; onClick: () => void };

type ToastItem = {
  id: number;
  message: string;
  action?: ToastAction;
};

type ToastContextValue = {
  pushToast: (message: string, action?: ToastAction) => void;
};

const ToastContext = createContext<ToastContextValue | null>(null);

export function useToast() {
  const ctx = useContext(ToastContext);
  if (!ctx) throw new Error("useToast must be used within ToastProvider");
  return ctx;
}

export function ToastProvider({ children }: { children: ReactNode }) {
  const [items, setItems] = useState<ToastItem[]>([]);

  const pushToast = useCallback((message: string, action?: ToastAction) => {
    const id = Date.now() + Math.random();
    setItems((prev) => [...prev, { id, message, action }]);
    window.setTimeout(() => {
      setItems((prev) => prev.filter((t) => t.id !== id));
    }, action ? 7000 : 4000);
  }, []);

  const value = useMemo(() => ({ pushToast }), [pushToast]);

  return (
    <ToastContext.Provider value={value}>
      {children}
      <div className="pointer-events-none fixed right-4 bottom-20 z-50 flex flex-col gap-2 md:bottom-4">
        {items.map((t) => (
          <div
            key={t.id}
            className={cn(
              "pointer-events-auto flex items-center gap-3 rounded-card border border-hairline bg-raised px-3 py-2 text-sm text-ink shadow-[0_8px_24px_rgb(10_16_14/0.28)]",
            )}
          >
            <span>{t.message}</span>
            {t.action && (
              <button
                type="button"
                className="text-mint"
                onClick={() => {
                  t.action?.onClick();
                  setItems((prev) => prev.filter((row) => row.id !== t.id));
                }}
              >
                {t.action.label}
              </button>
            )}
          </div>
        ))}
      </div>
    </ToastContext.Provider>
  );
}
