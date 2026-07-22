import { useEffect, useRef, type RefObject } from "react";

type DismissibleRef = RefObject<HTMLElement | null>;

/**
 * Dismiss on pointerdown outside `refs` and on Escape.
 * When `enabled` is false, listeners are inactive (e.g. while a modal owns focus).
 */
export function useDismissible({
  enabled,
  onDismiss,
  refs,
}: {
  enabled: boolean;
  onDismiss: () => void;
  refs: DismissibleRef[];
}) {
  const refsRef = useRef(refs);
  refsRef.current = refs;
  const onDismissRef = useRef(onDismiss);
  onDismissRef.current = onDismiss;

  useEffect(() => {
    if (!enabled) return;

    function isInside(target: EventTarget | null): boolean {
      if (!(target instanceof Node)) return false;
      return refsRef.current.some((ref) => {
        const el = ref.current;
        return el != null && el.contains(target);
      });
    }

    function onPointerDown(e: PointerEvent) {
      if (isInside(e.target)) return;
      onDismissRef.current();
    }

    function onKeyDown(e: KeyboardEvent) {
      if (e.key !== "Escape") return;
      e.preventDefault();
      onDismissRef.current();
    }

    // Capture phase so we see the event before stopPropagation on nested UI.
    document.addEventListener("pointerdown", onPointerDown, true);
    window.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown, true);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [enabled]);
}
