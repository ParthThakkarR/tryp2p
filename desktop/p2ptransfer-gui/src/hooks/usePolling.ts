import { useEffect, useRef } from "react";

/**
 * Polls `fn` immediately on mount and then every `intervalMs` milliseconds.
 * Cleans up the interval on unmount. If `enabled` is false, polling is paused.
 */
export function usePolling(
  fn: () => void,
  intervalMs: number,
  enabled = true
): void {
  const fnRef = useRef(fn);
  fnRef.current = fn;

  useEffect(() => {
    if (!enabled) return;

    // Fire immediately
    fnRef.current();

    const id = setInterval(() => {
      fnRef.current();
    }, intervalMs);

    return () => clearInterval(id);
  }, [intervalMs, enabled]);
}
