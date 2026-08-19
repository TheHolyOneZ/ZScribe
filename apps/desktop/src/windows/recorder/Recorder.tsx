import { useEffect, useReducer, useRef, useState } from "react";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { ChevronDown, Loader2, MicOff, Pause, Play, Square } from "lucide-react";

import { Waveform } from "@/components/Waveform";
import { cn } from "@/lib/cn";
import { useEta } from "@/lib/eta";
import { clock } from "@/lib/format";
import {
  EVENTS,
  ipc,
  on,
  SLOWDOWN_NOTE,
  stageLabel,
  type AppSettings,
  type LiveTranscript,
  type RecordingStatus,
  type StageProgress,
} from "@/lib/ipc";

const IDLE: RecordingStatus = {
  active: false,
  paused: false,
  durationMs: 0,
  level: { rms: 0, peak: 0 },
  problem: null,
  rewindMs: 0,
};


const WIDTH = 384;


const RECORDING_HEIGHT = 96;
const WORKING_HEIGHT = 96;
const TRANSCRIPT_HEIGHT = 176;


export function Recorder() {
  const [status, setStatus] = useState<RecordingStatus>(IDLE);
  const [progress, setProgress] = useState<StageProgress | null>(null);


  const [processing, setProcessing] = useState(false);
  const [live, setLive] = useState<LiveTranscript | null>(null);


  const [expanded, setExpanded] = useState(true);
  const [settings, setSettings] = useState<AppSettings | null>(null);

  const scroller = useRef<HTMLDivElement>(null);

  useEffect(() => {
    void ipc.recordingStatus().then(setStatus);
    void ipc.getSettings().then(setSettings);

    const listeners = [
      on<RecordingStatus>(EVENTS.recordingLevel, setStatus),


      on<LiveTranscript>(EVENTS.liveTranscript, (next) =>
        setLive((previous) =>
          next.text ? next : { ...next, text: previous?.text ?? "" },
        ),
      ),
      on<AppSettings>(EVENTS.settingsChanged, setSettings),
      on<StageProgress>(EVENTS.transcribeProgress, setProgress),
      on<StageProgress>(EVENTS.summarizeProgress, setProgress),
      on<string>(EVENTS.recordingStarted, () => {
        setLive(null);
        setProgress(null);
        setProcessing(false);
      }),
      on<string>(EVENTS.recordingStopped, () => {
        void ipc.recordingStatus().then(setStatus);
        setProcessing(true);
      }),


      on<string>(EVENTS.recordingReady, () => {
        setProcessing(false);
        setProgress(null);
      }),
      on<unknown>(EVENTS.pipelineFailed, () => {
        setProcessing(false);
        setProgress(null);
      }),
    ];

    return () => {
      for (const listener of listeners) void listener.then((off) => off());
    };
  }, []);

  const liveEnabled = settings?.transcription.liveTranscript ?? false;
  const working = processing && !status.active;
  const showTranscript = expanded && liveEnabled && status.active;
  const silentFor = useSilence(status);


  useEffect(() => {
    const height = working
      ? WORKING_HEIGHT
      : showTranscript
        ? RECORDING_HEIGHT + TRANSCRIPT_HEIGHT
        : RECORDING_HEIGHT;

    void getCurrentWindow().setSize(new LogicalSize(WIDTH, height));
  }, [showTranscript, working]);

  useEffect(() => {
    const root = document.documentElement;
    const theme = settings?.appearance.theme ?? "system";

    if (theme !== "system") {
      root.setAttribute("data-theme", theme);
      return;
    }

    const query = window.matchMedia("(prefers-color-scheme: light)");
    const apply = () => root.setAttribute("data-theme", query.matches ? "light" : "dark");

    apply();
    query.addEventListener("change", apply);
    return () => query.removeEventListener("change", apply);
  }, [settings?.appearance.theme]);

  const [scrolled, setScrolled] = useState(false);

  useEffect(() => {
    const element = scroller.current;
    if (!element) return;

    element.scrollTop = element.scrollHeight;


    setScrolled(element.scrollHeight > element.clientHeight + 1);
  }, [live?.text, showTranscript]);

  const opacity = (settings?.appearance.recorderOpacity ?? 92) / 100;
  const sources = settings?.recording.sources.filter((source) => source.enabled) ?? [];

  return (
    <div
      className={cn(
        "recorder-shell relative flex h-full flex-col overflow-hidden rounded-2xl select-none",
        "border border-line bg-surface shadow-[var(--shadow-overlay)]",


        status.problem && !working && "border-danger/60",
      )}
      style={{ opacity }}
    >
      {working ? <Working progress={progress} /> : null}

      {!working ? (
        <div data-tauri-drag-region className="flex shrink-0 flex-col gap-2.5 px-3.5 pt-3 pb-3.5">
          <div className="flex items-center gap-2.5" data-tauri-drag-region>
            <RecordingLight paused={status.paused} problem={status.problem !== null} />

            <span data-numeric className="text-xl leading-none font-medium text-fg tabular-nums">
              {clock(status.durationMs)}
            </span>


            <div
              className="flex min-w-0 flex-1 items-baseline gap-1.5 text-2xs"
              data-tauri-drag-region
            >
              <span className="truncate text-faint">{describe(sources)}</span>
              <Verdict status={status} silentFor={silentFor} />
            </div>

            {liveEnabled ? (
              <BarButton
                label={showTranscript ? "Hide the transcript" : "Show the transcript"}
                onClick={() => setExpanded((open) => !open)}
              >
                <ChevronDown
                  className={cn(
                    "transition-transform duration-base ease-out",
                    showTranscript && "rotate-180",
                  )}
                />
              </BarButton>
            ) : null}

            <BarButton
              label={status.paused ? "Resume" : "Pause"}
              onClick={() => void (status.paused ? ipc.resumeRecording() : ipc.pauseRecording())}
            >
              {status.paused ? <Play /> : <Pause />}
            </BarButton>


            <button
              type="button"
              aria-label="Stop recording"
              title="Stop recording"
              onClick={() => void ipc.stopRecording()}
              className={cn(
                "inline-flex h-7 items-center gap-1.5 rounded-lg pr-2.5 pl-2",
                "bg-danger text-xs font-medium text-white ring-1 ring-white/10 ring-inset",
                "transition-colors duration-fast ease-out hover:bg-danger-hover",
                "[&_svg]:size-3 [&_svg]:fill-current",
              )}
            >
              <Square />
              Stop
            </button>
          </div>


          <Waveform
            level={status.paused ? IDLE.level : status.level}
            running={status.active && !status.paused}
            className="h-8"
          />
        </div>
      ) : null}

      {showTranscript ? (
        <div className="flex min-h-0 flex-1 flex-col border-t border-line-subtle">
          <div className="flex items-center justify-between px-4 py-1.5">
            <span className="text-2xs font-medium tracking-wide text-faint uppercase">
              Live transcript
            </span>
            {live?.working ? (
              <span className="inline-flex items-center gap-1.5 text-2xs text-faint [&_svg]:size-3">
                <Loader2 className="animate-spin" />
                Listening
              </span>
            ) : null}
          </div>


          <div
            ref={scroller}
            data-selectable
            className={cn("scroll-area min-h-0 flex-1 px-4 pb-3", scrolled && "fade-top")}
          >
            {live?.text ? (
              <p className="text-sm leading-relaxed text-fg">{live.text}</p>
            ) : (
              <p className="text-xs leading-relaxed text-faint">
                Text appears a few seconds after it is spoken, and rewrites itself as more of the
                sentence arrives. The saved transcript is made once recording stops.
              </p>
            )}
          </div>
        </div>
      ) : null}
    </div>
  );
}


function RecordingLight({ paused, problem }: { paused: boolean; problem: boolean }) {
  if (problem) {
    return <MicOff aria-hidden className="size-3.5 shrink-0 text-danger" />;
  }
  if (paused) {
    return <Pause aria-hidden className="size-3.5 shrink-0 fill-current text-muted" />;
  }

  return (
    <span aria-hidden className="relative flex size-2.5 shrink-0 items-center justify-center">

      <span className="absolute inset-0 animate-recording rounded-full bg-recording/30 blur-[3px]" />
      <span className="relative size-2.5 rounded-full bg-recording" />
    </span>
  );
}


const SILENCE_FLOOR = 0.001;


function useSilence(status: RecordingStatus): number | null {
  const [quietSince, setQuietSince] = useState<number | null>(null);
  const [, tick] = useReducer((count: number) => count + 1, 0);

  useEffect(() => {
    if (!status.active || status.paused) {
      setQuietSince(null);
      return;
    }
    if (status.level.peak >= SILENCE_FLOOR) setQuietSince(null);
    else setQuietSince((since) => since ?? Date.now());
  }, [status.level, status.active, status.paused]);


  useEffect(() => {
    if (quietSince === null) return;
    const id = setInterval(tick, 500);
    return () => clearInterval(id);
  }, [quietSince]);

  return quietSince === null ? null : Date.now() - quietSince;
}


const SILENCE_GRACE_MS = 4000;


function Verdict({
  status,
  silentFor,
}: {
  status: RecordingStatus;
  silentFor: number | null;
}) {
  const say = (text: string, tone: string) => (
    <>
      <span aria-hidden className="shrink-0 text-faint">
        ·
      </span>
      <span className={cn("shrink-0 font-medium", tone)}>{text}</span>
    </>
  );

  if (status.problem) return say("Microphone lost", "text-danger");
  if (status.paused) return say("Paused", "text-muted");
  if (!status.active) return null;


  if (status.level.peak >= 0.99) return say("Too loud", "text-level-clip");
  if (silentFor !== null && silentFor >= SILENCE_GRACE_MS) {
    return say("No sound arriving", "text-level-mid");
  }
  return null;
}


function Working({ progress }: { progress: StageProgress | null }) {
  const eta = useEta(
    progress?.percent ?? null,
    progress ? `${progress.stage}:${progress.recordingId}` : "",
  );

  return (
    <div data-tauri-drag-region className="flex flex-1 flex-col justify-center gap-2 px-4 py-3">
      <div className="flex items-center gap-2.5" data-tauri-drag-region>
        <Loader2 className="size-3.5 shrink-0 animate-spin text-accent" />
        <span className="text-sm font-medium text-fg">
          {progress ? stageLabel(progress) : "Transcribing"}
        </span>
        <div className="flex-1" />
        {progress ? (
          <span data-numeric className="text-xs text-faint tabular-nums">
            {eta ? `${eta} · ` : ""}
            {progress.percent}%
          </span>
        ) : null}
      </div>

      <div className="h-1 overflow-hidden rounded-full bg-inset">
        <div
          className={cn(
            "h-full rounded-full bg-accent",


            progress ? "transition-[width] duration-base ease-out" : "w-1/3 animate-skeleton",
          )}
          style={progress ? { width: `${progress.percent}%` } : undefined}
        />
      </div>


      {!progress || progress.onThisMachine ? (
        <p className="text-2xs leading-relaxed text-faint">{SLOWDOWN_NOTE}</p>
      ) : null}
    </div>
  );
}


function describe(sources: { name: string; kind: string }[]): string {

  if (sources.length === 0) return "Microphone";

  if (sources.length === 1) {
    const only = sources[0]!;
    if (only.name) return only.name;
    return only.kind === "systemAudio" ? "System audio" : "Microphone";
  }

  return `${sources.length} sources`;
}

function BarButton({
  label,
  onClick,
  children,
}: {
  label: string;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={onClick}
      className={cn(
        "inline-flex size-7 items-center justify-center rounded-lg",
        "text-muted transition-colors duration-fast ease-out",
        "hover:bg-hover hover:text-fg",
        "[&_svg]:size-3.5",
      )}
    >
      {children}
    </button>
  );
}
