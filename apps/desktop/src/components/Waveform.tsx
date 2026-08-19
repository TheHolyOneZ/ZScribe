import { useEffect, useRef, useState } from "react";

import { cn } from "@/lib/cn";
import type { Level } from "@/lib/ipc";


const BARS = 64;


const TICK_MS = 90;


function amplitude(level: Level): number {
  return Math.min(1, Math.sqrt(level.rms) * 1.2);
}


export function Waveform({
  level,
  running,
  className,
}: {
  level: Level;

  running: boolean;
  className?: string;
}) {
  const [history, setHistory] = useState<number[]>(() => new Array<number>(BARS).fill(0));


  const latest = useRef(level);
  latest.current = level;

  const clipping = level.peak >= 0.99;

  useEffect(() => {
    if (!running) return;

    const id = setInterval(() => {
      setHistory((previous) => [...previous.slice(1), amplitude(latest.current)]);
    }, TICK_MS);

    return () => clearInterval(id);
  }, [running]);

  return (
    <div
      className={cn("waveform flex h-full items-center gap-[2px]", className)}
      role="meter"
      aria-label="Input level"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(level.rms * 100)}
    >
      {history.map((value, index) => (
        <span
          key={index}
          aria-hidden


          style={{ height: `${Math.max(2, Math.round(value * 100))}%` }}
          className={cn(
            "min-w-px flex-1 rounded-full",


            clipping ? "bg-level-clip" : "bg-fg/70",
            !running && "opacity-40",
          )}
        />
      ))}
    </div>
  );
}
