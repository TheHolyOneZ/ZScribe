import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  ChevronDown,
  ChevronUp,
  Minus,
  Pin,
  PinOff,
  Plus,
  Search,
  X,
} from "lucide-react";

import { Player, type PlayerHandle } from "@/components/Player";
import { TranscriptView, type TranscriptSize } from "@/components/TranscriptView";
import { EmptyState, Input, ResizeEdges, Skeleton } from "@/components/ui";
import { cn } from "@/lib/cn";
import { lineAt } from "@/lib/transcript";
import {
  EVENTS,
  ipc,
  on,
  type AppSettings,
  type PlayerOpen,
  type RecordingDetail,
} from "@/lib/ipc";


const SIZES: TranscriptSize[] = ["normal", "large", "larger"];


export function PlayerWindow() {
  const [detail, setDetail] = useState<RecordingDetail | null>(null);
  const [loading, setLoading] = useState(true);
  const [spokenIndex, setSpokenIndex] = useState(-1);
  const [needle, setNeedle] = useState("");
  const [size, setSize] = useState<TranscriptSize>("large");
  const [pinned, setPinned] = useState(false);

  const player = useRef<PlayerHandle>(null);
  const finder = useRef<HTMLInputElement>(null);

  const load = useCallback(async (opening: PlayerOpen) => {
    setLoading(true);
    setSpokenIndex(-1);
    setDetail(await ipc.getRecording(opening.id));
    setLoading(false);


    if (opening.atMs !== null) {
      requestAnimationFrame(() => player.current?.seek(opening.atMs ?? 0));
    }
  }, []);


  useEffect(() => {
    void ipc.playerRecording().then((opening) => {
      if (opening) void load(opening);
      else setLoading(false);
    });

    const listeners = [
      on<PlayerOpen>(EVENTS.playerOpen, (opening) => void load(opening)),


      on<unknown>(EVENTS.playerHidden, () => player.current?.pause()),
      on<AppSettings>(EVENTS.settingsChanged, (settings) => applyTheme(settings)),
    ];

    void ipc.getSettings().then(applyTheme);

    return () => {
      for (const listener of listeners) void listener.then((off) => off());
    };
  }, [load]);

  const segments = detail?.transcript?.segments;

  const followAlong = useCallback(
    (ms: number) => setSpokenIndex(segments ? lineAt(segments, ms) : -1),
    [segments],
  );


  const matches = useMemo(() => {
    const wanted = needle.trim().toLowerCase();
    if (!wanted || !segments) return [];

    return segments
      .map((segment, index) => ({ segment, index }))
      .filter(({ segment }) => segment.text.toLowerCase().includes(wanted));
  }, [needle, segments]);

  const jump = useCallback(
    (direction: 1 | -1) => {
      if (matches.length === 0) return;


      const from = matches.findIndex(({ index }) => index > spokenIndex);
      const at =
        direction === 1
          ? from === -1
            ? 0
            : from
          : from <= 0
            ? matches.length - 1
            : from - 1;

      const target = matches[at];
      if (target) player.current?.seek(target.segment.startMs);
    },
    [matches, spokenIndex],
  );


  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      const typing =
        target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement;

      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "f") {
        event.preventDefault();
        finder.current?.focus();
        finder.current?.select();
        return;
      }

      if (event.key === "Escape") {
        if (typing) {
          setNeedle("");
          finder.current?.blur();
        } else {
          void getCurrentWindow().hide();
        }
        return;
      }

      if (typing) {
        if (event.key === "Enter") {
          event.preventDefault();
          jump(event.shiftKey ? -1 : 1);
        }
        return;
      }

      if (event.key === " ") {
        event.preventDefault();
        player.current?.toggle();
      }
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [jump]);

  const step = (by: 1 | -1) =>
    setSize((current) => {
      const at = SIZES.indexOf(current) + by;
      return SIZES[Math.max(0, Math.min(SIZES.length - 1, at))] ?? current;
    });

  return (
    <div className="window-shell flex h-full flex-col">


      <ResizeEdges />

      <header
        data-tauri-drag-region
        className="flex shrink-0 items-center gap-2 border-b border-line-subtle px-2.5 py-2"
      >
        <p
          data-tauri-drag-region
          className="pointer-events-none min-w-0 flex-1 truncate text-xs font-medium text-fg"
        >
          {detail?.recording.title ?? "Player"}
        </p>

        <button
          type="button"
          onClick={() => {
            const next = !pinned;
            setPinned(next);
            void getCurrentWindow().setAlwaysOnTop(next);
          }}
          aria-pressed={pinned}
          title={pinned ? "Let other windows cover this" : "Keep this above other windows"}
          className={cn(
            "inline-flex size-7 shrink-0 items-center justify-center rounded-md",
            "transition-colors duration-fast ease-out [&_svg]:size-3.5",
            pinned ? "bg-accent-subtle text-accent" : "text-muted hover:bg-hover hover:text-fg",
          )}
        >
          {pinned ? <Pin /> : <PinOff />}
        </button>

        <button
          type="button"
          onClick={() => void getCurrentWindow().hide()}
          aria-label="Close"
          title="Close"
          className={cn(
            "inline-flex size-7 shrink-0 items-center justify-center rounded-md",
            "text-muted transition-colors duration-fast ease-out",
            "hover:bg-danger hover:text-on-accent [&_svg]:size-3.5",
          )}
        >
          <X />
        </button>
      </header>

      {loading ? (
        <div className="space-y-3 p-4">
          <Skeleton className="h-12 w-full" />
          <Skeleton className="h-3 w-1/3" />
          <Skeleton className="h-24 w-full" />
        </div>
      ) : !detail ? (
        <EmptyState
          title="Nothing to play"
          description="Open a recording from the library and it appears here."
        />
      ) : (
        <>
          <div className="shrink-0 px-3 pt-3">
            {detail.recording.audioPath ? (
              <Player
                ref={player}
                recordingId={detail.recording.id}
                durationMs={detail.recording.durationMs}
                onTime={followAlong}
              />
            ) : (
              <p className="mb-3 rounded-lg border border-line-subtle bg-surface px-3 py-2 text-2xs text-muted">
                The audio for this recording was deleted. The transcript is still here.
              </p>
            )}

            <div className="mb-2 flex items-center gap-2">
              <div className="relative min-w-0 flex-1">
                <Search className="pointer-events-none absolute top-1/2 left-2 size-3 -translate-y-1/2 text-faint" />
                <Input
                  ref={finder}
                  value={needle}
                  onChange={(event) => setNeedle(event.target.value)}
                  placeholder="Find in this transcript"
                  aria-label="Find in this transcript"
                  spellCheck={false}
                  className="pl-6 text-xs"
                />
              </div>

              <span data-numeric className="w-16 shrink-0 text-right text-2xs text-faint">
                {needle.trim() ? `${matches.length} found` : ""}
              </span>

              <div className="flex shrink-0 items-center">
                <IconButton
                  label="Previous match"
                  disabled={matches.length === 0}
                  onClick={() => jump(-1)}
                >
                  <ChevronUp />
                </IconButton>
                <IconButton
                  label="Next match"
                  disabled={matches.length === 0}
                  onClick={() => jump(1)}
                >
                  <ChevronDown />
                </IconButton>
              </div>

              <div className="flex shrink-0 items-center">
                <IconButton
                  label="Smaller text"
                  disabled={size === SIZES[0]}
                  onClick={() => step(-1)}
                >
                  <Minus />
                </IconButton>
                <IconButton
                  label="Larger text"
                  disabled={size === SIZES[SIZES.length - 1]}
                  onClick={() => step(1)}
                >
                  <Plus />
                </IconButton>
              </div>
            </div>
          </div>

          <div className="scroll-area min-h-0 flex-1 px-3 pb-4">
            {detail.transcript ? (
              <TranscriptView
                transcript={detail.transcript}
                spokenIndex={spokenIndex}
                onSeek={
                  detail.recording.audioPath ? (ms) => player.current?.playFrom(ms) : null
                }
                onEdit={null}


                onRenameSpeaker={async (from, to) => {
                  await ipc.renameSpeaker(detail.recording.id, from, to);
                  setDetail(await ipc.getRecording(detail.recording.id));
                }}
                size={size}
                highlight={needle}
                stickyClassName="bg-canvas/95"
              />
            ) : (
              <EmptyState
                title="No transcript"
                description="This recording has not been transcribed yet."
              />
            )}
          </div>
        </>
      )}
    </div>
  );
}

function IconButton({
  label,
  disabled,
  onClick,
  children,
}: {
  label: string;
  disabled?: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      aria-label={label}
      title={label}
      className={cn(
        "inline-flex size-7 items-center justify-center rounded-md text-muted",
        "transition-colors duration-fast ease-out hover:bg-hover hover:text-fg",
        "disabled:pointer-events-none disabled:opacity-30 [&_svg]:size-3.5",
      )}
    >
      {children}
    </button>
  );
}


function applyTheme(settings: AppSettings) {
  const theme = settings.appearance.theme;
  const root = document.documentElement;

  if (theme !== "system") {
    root.setAttribute("data-theme", theme);
    return;
  }
  root.setAttribute(
    "data-theme",
    window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark",
  );
}
