import { useEffect, useRef, useState } from "react";


export function formatEta(ms: number): string {
  const total = Math.max(0, Math.round(ms / 1000));
  if (total < 1) return "almost done";
  if (total < 60) return `~${total}s left`;

  const minutes = Math.floor(total / 60);
  const seconds = total % 60;
  if (minutes < 60) {
    return seconds === 0 ? `~${minutes}m left` : `~${minutes}m ${seconds}s left`;
  }

  const hours = Math.floor(minutes / 60);
  const restMinutes = minutes % 60;
  return restMinutes === 0 ? `~${hours}h left` : `~${hours}h ${restMinutes}m left`;
}


export function useEta(percent: number | null | undefined, key: string): string | null {

  const start = useRef<{ key: string; time: number; percent: number } | null>(null);

  const deadline = useRef<number | null>(null);

  const [, tick] = useState(0);

  useEffect(() => {
    if (percent === null || percent === undefined) {
      start.current = null;
      deadline.current = null;
      return;
    }

    const now = Date.now();


    if (!start.current || start.current.key !== key || percent < start.current.percent - 1) {
      start.current = { key, time: now, percent };
      deadline.current = null;
      return;
    }

    const gainedPercent = percent - start.current.percent;
    const elapsed = now - start.current.time;


    if (gainedPercent > 0 && elapsed > 500) {
      const perMs = gainedPercent / elapsed;
      deadline.current = now + (100 - percent) / perMs;
    }
  }, [percent, key]);

  useEffect(() => {
    if (percent === null || percent === undefined || percent >= 100) return;
    const id = setInterval(() => tick((n) => n + 1), 1000);
    return () => clearInterval(id);
  }, [percent]);

  if (percent === null || percent === undefined || percent >= 100) return null;
  if (deadline.current === null) return null;

  const remaining = deadline.current - Date.now();


  if (remaining < 1500) return "almost done";
  return formatEta(remaining);
}
