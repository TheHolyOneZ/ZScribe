import { forwardRef, useEffect, useImperativeHandle, useMemo, useRef, useState } from "react";
import { Gauge, Pause, Play, RotateCcw, RotateCw } from "lucide-react";

import { cn } from "@/lib/cn";
import { clock } from "@/lib/format";
import { ipc } from "@/lib/ipc";
import { useAppStore } from "@/store/useAppStore";


const BARS = 480;


const SPEEDS = [1, 1.25, 1.5, 2, 0.75] as const;


const NUDGE_MS = 5_000;

export interface PlayerHandle {

  seek: (ms: number) => void;


  playFrom: (ms: number) => void;


  toggle: () => void;


  pause: () => void;
}

interface PlayerProps {
  recordingId: string;
  durationMs: number;


  onTime: (ms: number) => void;
}


export const Player = forwardRef<PlayerHandle, PlayerProps>(function Player(
  { recordingId, durationMs, onTime },
  ref,
) {
  const audio = useRef<HTMLAudioElement>(null);
  const [playing, setPlaying] = useState(false);
  const [currentMs, setCurrentMs] = useState(0);
  const [peaks, setPeaks] = useState<number[]>([]);
  const [speed, setSpeed] = useState(1);
  const [failed, setFailed] = useState(false);
  const [src, setSrc] = useState<string | null>(null);

  const displayServer = useAppStore((state) => state.capabilities?.displayServer ?? null);


  const [loadedMs, setLoadedMs] = useState<number | null>(null);
  const totalMs = loadedMs ?? durationMs;

  useEffect(() => {
    setLoadedMs(null);
    setCurrentMs(0);
    setPlaying(false);
    setFailed(false);
    setSrc(null);
    onTime(0);

    let cancelled = false;
    void ipc
      .audioUrl(recordingId)
      .then((url) => {
        if (cancelled) return;


        if (url) setSrc(url);
        else setFailed(true);
      })
      .catch(() => {
        if (!cancelled) setFailed(true);
      });

    return () => {
      cancelled = true;
    };

  }, [recordingId]);

  useEffect(() => {
    let cancelled = false;
    setPeaks([]);

    void ipc
      .audioPeaks(recordingId, BARS)
      .then((next) => {
        if (!cancelled) setPeaks(next);
      })
      .catch(() => {

        if (!cancelled) setPeaks([]);
      });

    return () => {
      cancelled = true;
    };
  }, [recordingId]);

  const move = (ms: number, thenPlay: boolean) => {
    const element = audio.current;
    if (!element || !src) return;

    const clamped = Math.max(0, Math.min(ms, totalMs));
    element.currentTime = clamped / 1000;
    setCurrentMs(clamped);
    onTime(clamped);

    if (thenPlay) void element.play().catch(() => setFailed(true));
  };

  useImperativeHandle(ref, () => ({
    seek: (ms) => move(ms, false),
    playFrom: (ms) => move(ms, true),
    toggle,
    pause: () => audio.current?.pause(),
  }));

  const toggle = () => {
    const element = audio.current;
    if (!element || !src) return;

    if (element.paused) void element.play().catch(() => setFailed(true));
    else element.pause();
  };

  const cycleSpeed = () => {
    const next = SPEEDS[(SPEEDS.indexOf(speed as (typeof SPEEDS)[number]) + 1) % SPEEDS.length]!;
    setSpeed(next);
    if (audio.current) audio.current.playbackRate = next;
  };

  const progress = totalMs > 0 ? Math.min(100, (currentMs / totalMs) * 100) : 0;


  const bars = useMemo(
    () => (
      <svg
        viewBox={`0 0 ${Math.max(peaks.length, 1)} 100`}
        preserveAspectRatio="none"
        className="h-full w-full"
        aria-hidden
      >
        {peaks.map((level, index) => {


          const height = Math.max(2, (level / 255) * 96);
          return (
            <rect
              key={index}
              x={index + 0.15}
              width={0.7}
              y={50 - height / 2}
              height={height}
              rx={0.3}
              fill="currentColor"
            />
          );
        })}
      </svg>
    ),
    [peaks],
  );

  const scrub = (event: React.PointerEvent<HTMLDivElement>) => {
    const box = event.currentTarget.getBoundingClientRect();
    if (box.width <= 0) return;

    const fraction = Math.max(0, Math.min(1, (event.clientX - box.left) / box.width));
    move(fraction * totalMs, false);
  };

  return (
    <div className="mb-5 rounded-lg border border-line-subtle bg-surface px-3 py-2.5">
      <audio
        ref={audio}


        {...(src ? { src } : {})}
        preload="metadata"
        onPlay={() => setPlaying(true)}
        onPause={() => setPlaying(false)}
        onEnded={() => setPlaying(false)}
        onError={() => setFailed(true)}
        onLoadedMetadata={(event) => {
          const seconds = event.currentTarget.duration;


          if (Number.isFinite(seconds) && seconds > 0) setLoadedMs(seconds * 1000);
        }}
        onTimeUpdate={(event) => {
          const ms = event.currentTarget.currentTime * 1000;
          setCurrentMs(ms);
          onTime(ms);
        }}
      />

      <div className="flex items-center gap-2.5">
        <button
          type="button"
          onClick={toggle}
          disabled={failed || !src}
          aria-label={playing ? "Pause" : "Play"}
          title={playing ? "Pause" : "Play"}
          className={cn(
            "inline-flex size-8 shrink-0 items-center justify-center rounded-full",
            "bg-accent text-on-accent transition-opacity duration-fast ease-out",
            "hover:opacity-90 disabled:opacity-40 [&_svg]:size-3.5",
          )}
        >
          {playing ? <Pause /> : <Play className="translate-x-px" />}
        </button>

        <button
          type="button"
          onClick={() => move(currentMs - NUDGE_MS, false)}
          disabled={failed || !src}
          aria-label="Back five seconds"
          title="Back five seconds"
          className={cn(
            "inline-flex size-7 shrink-0 items-center justify-center rounded-md text-muted",
            "transition-colors duration-fast ease-out hover:bg-hover hover:text-fg",
            "disabled:opacity-40 [&_svg]:size-3.5",
          )}
        >
          <RotateCcw />
        </button>
        <button
          type="button"
          onClick={() => move(currentMs + NUDGE_MS, false)}
          disabled={failed || !src}
          aria-label="Forward five seconds"
          title="Forward five seconds"
          className={cn(
            "inline-flex size-7 shrink-0 items-center justify-center rounded-md text-muted",
            "transition-colors duration-fast ease-out hover:bg-hover hover:text-fg",
            "disabled:opacity-40 [&_svg]:size-3.5",
          )}
        >
          <RotateCw />
        </button>


        <div
          role="slider"
          tabIndex={failed || !src ? -1 : 0}
          aria-label="Position in the recording"
          aria-valuemin={0}
          aria-valuemax={Math.round(totalMs / 1000)}
          aria-valuenow={Math.round(currentMs / 1000)}
          aria-valuetext={clock(currentMs)}
          onPointerDown={(event) => {
            if (failed || !src) return;
            event.currentTarget.setPointerCapture(event.pointerId);
            scrub(event);
          }}
          onPointerMove={(event) => {
            if (event.currentTarget.hasPointerCapture(event.pointerId)) scrub(event);
          }}
          onKeyDown={(event) => {
            const step =
              event.key === "ArrowRight" ? NUDGE_MS : event.key === "ArrowLeft" ? -NUDGE_MS : null;
            if (step !== null) {
              event.preventDefault();
              move(currentMs + step, false);
              return;
            }
            if (event.key === "Home") {
              event.preventDefault();
              move(0, false);
            } else if (event.key === "End") {
              event.preventDefault();
              move(totalMs, false);
            } else if (event.key === " " || event.key === "Enter") {
              event.preventDefault();
              toggle();
            }
          }}
          className={cn(
            "relative h-9 min-w-0 flex-1 cursor-pointer touch-none rounded",
            "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent",
            (failed || !src) && "pointer-events-none opacity-40",
          )}
        >
          {peaks.length > 0 ? (
            <>
              <div className="absolute inset-0 text-line-strong">{bars}</div>
              <div
                className="absolute inset-0 text-accent"
                style={{ clipPath: `inset(0 ${100 - progress}% 0 0)` }}
              >
                {bars}
              </div>
            </>
          ) : (


            <div className="absolute inset-x-0 top-1/2 h-1 -translate-y-1/2 overflow-hidden rounded-full bg-inset">
              <div className="h-full rounded-full bg-accent" style={{ width: `${progress}%` }} />
            </div>
          )}

          <div
            aria-hidden
            className="pointer-events-none absolute top-0 bottom-0 w-px bg-accent"
            style={{ left: `${progress}%` }}
          />
        </div>

        <span data-numeric className="shrink-0 text-2xs text-faint tabular-nums">
          {clock(currentMs)} / {clock(totalMs)}
        </span>

        <button
          type="button"
          onClick={cycleSpeed}
          disabled={failed || !src}
          aria-label={`Playback speed ${speed}×`}
          title="Playback speed"
          className={cn(
            "inline-flex h-7 shrink-0 items-center gap-1 rounded-md px-1.5 text-2xs text-muted",
            "transition-colors duration-fast ease-out hover:bg-hover hover:text-fg",
            "disabled:opacity-40 [&_svg]:size-3",
          )}
        >
          <Gauge />
          {speed}×
        </button>
      </div>

      {failed ? (
        <p className="mt-2 text-2xs leading-relaxed text-warning">
          This recording&rsquo;s audio would not play. The file may have been moved or deleted
          {displayServer === "x11" || displayServer === "wayland"
            ? " — and on Linux the webview plays sound through GStreamer, so `gst-plugins-good` has to be installed"
            : ""}
          .
        </p>
      ) : null}
    </div>
  );
});
