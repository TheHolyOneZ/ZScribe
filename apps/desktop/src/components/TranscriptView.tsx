import { useEffect, useRef, useState } from "react";
import { Copy, FileText, Locate, LocateFixed, Pencil } from "lucide-react";

import { ContextMenu, Input, MenuItem, MenuSeparator, Textarea } from "@/components/ui";
import { CopySelection, copyText, useSelection } from "@/components/selection";
import { cn } from "@/lib/cn";
import { clock } from "@/lib/format";
import type { Segment, Transcript } from "@/lib/ipc";


export type TranscriptSize = "normal" | "large" | "larger";

const SIZES: Record<TranscriptSize, string> = {
  normal: "text-sm",
  large: "text-base",
  larger: "text-lg",
};


export function TranscriptView({
  transcript,
  spokenIndex,
  onSeek,
  onEdit,
  onRenameSpeaker,
  size = "normal",
  highlight = "",
  stickyClassName,
}: {
  transcript: Transcript;
  spokenIndex: number;


  onSeek: ((ms: number) => void) | null;


  onEdit: ((index: number, text: string) => Promise<void>) | null;


  onRenameSpeaker?: ((from: string, to: string) => Promise<void>) | null;

  size?: TranscriptSize;


  highlight?: string;


  stickyClassName?: string;
}) {
  const { selection, capture } = useSelection();
  const [line, setLine] = useState<Segment | null>(null);
  const [lineIndex, setLineIndex] = useState(-1);
  const [editing, setEditing] = useState(-1);
  const [draft, setDraft] = useState("");


  const [follow, setFollow] = useState(true);


  const [renaming, setRenaming] = useState<number | null>(null);
  const [name, setName] = useState("");

  const spoken = useRef<HTMLParagraphElement>(null);

  const save = async () => {
    const index = editing;
    const text = draft.trim();
    setEditing(-1);

    if (!onEdit || index < 0 || text === transcript.segments[index]?.text.trim()) return;
    await onEdit(index, text);
  };

  const whole = transcript.segments
    .map((segment) => `${clock(segment.startMs)}  ${segment.text.trim()}`)
    .join("\n");


  useEffect(() => {
    if (!follow) return;
    spoken.current?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }, [spokenIndex, follow]);

  return (
    <div>


      <div
        className={cn(
          "sticky top-0 z-10 -mx-1 mb-2 flex items-center justify-between gap-3 px-1 py-1.5",
          "backdrop-blur-sm",
          stickyClassName ?? "bg-canvas/90",
        )}
      >
        <p data-numeric className="truncate text-2xs text-faint">
          {transcript.language} · {transcript.model} · {transcript.segments.length} segments
        </p>

        <button
          type="button"
          onClick={() => setFollow((was) => !was)}
          aria-pressed={follow}
          title={
            follow
              ? "The page scrolls to keep up with the audio"
              : "The page stays where you leave it"
          }
          className={cn(
            "inline-flex shrink-0 items-center gap-1.5 rounded-md border px-2 py-1",
            "text-2xs transition-colors duration-fast ease-out [&_svg]:size-3",
            follow
              ? "border-accent-border bg-accent-subtle text-fg"
              : "border-line-subtle text-muted hover:bg-hover hover:text-fg",
          )}
        >
          {follow ? <LocateFixed /> : <Locate />}
          Follow the audio
        </button>
      </div>

      <ContextMenu
        content={
          <>
            <CopySelection selection={selection} />
            {onEdit ? (
              <>
                <MenuItem
                  icon={<Pencil />}
                  disabled={lineIndex < 0}
                  onSelect={() => {
                    if (lineIndex < 0) return;
                    setDraft(transcript.segments[lineIndex]?.text.trim() ?? "");
                    setEditing(lineIndex);
                  }}
                >
                  Correct this line
                </MenuItem>
                <MenuSeparator />
              </>
            ) : null}
            <MenuItem
              icon={<Copy />}
              disabled={!line}
              onSelect={() => line && copyText(line.text.trim())}
            >
              Copy this line
            </MenuItem>
            <MenuItem
              icon={<Copy />}
              disabled={!line}
              onSelect={() => line && copyText(`${clock(line.startMs)}  ${line.text.trim()}`)}
            >
              Copy it with the time
            </MenuItem>
            <MenuSeparator />
            <MenuItem icon={<FileText />} onSelect={() => copyText(whole)}>
              Copy the whole transcript
            </MenuItem>
          </>
        }
      >
        <div data-selectable className={cn("space-y-1", SIZES[size])}>
          {transcript.segments.map((segment, index) => {
            const speaking = index === spokenIndex;


            const speaker = segment.speaker ?? null;
            const opensATurn =
              speaker !== null && speaker !== (transcript.segments[index - 1]?.speaker ?? null);

            if (index === editing) {
              return (
                <div key={index} className="flex gap-3 px-1.5 py-1">
                  <span data-numeric className="w-12 shrink-0 pt-1.5 text-2xs text-accent">
                    {clock(segment.startMs)}
                  </span>
                  <Textarea
                    autoFocus
                    value={draft}
                    onChange={(event) => setDraft(event.target.value)}
                    onBlur={() => void save()}
                    onKeyDown={(event) => {


                      if (event.key === "Enter" && !event.shiftKey) {
                        event.preventDefault();
                        void save();
                      }
                      if (event.key === "Escape") {
                        event.preventDefault();
                        setEditing(-1);
                      }
                    }}
                    aria-label={`Transcript line at ${clock(segment.startMs)}`}
                    rows={Math.max(1, Math.ceil(draft.length / 70))}
                    className="min-w-0 flex-1 text-sm"
                  />
                </div>
              );
            }

            return (
              <div key={index}>
                {opensATurn ? (
                  <SpeakerName
                    name={speaker}
                    editing={renaming === index}
                    draft={name}
                    onDraft={setName}
                    onStart={
                      onRenameSpeaker
                        ? () => {
                            setName(speaker);
                            setRenaming(index);
                          }
                        : null
                    }
                    onSave={async () => {
                      const wanted = name.trim();
                      setRenaming(null);
                      if (wanted && wanted !== speaker) await onRenameSpeaker?.(speaker, wanted);
                    }}
                    onCancel={() => setRenaming(null)}
                  />
                ) : null}

              <p
                ref={speaking ? spoken : undefined}


                onContextMenu={() => {
                  setLine(segment);
                  setLineIndex(index);
                  capture();
                }}


                onDoubleClick={() => {
                  if (!onEdit) return;
                  setDraft(segment.text.trim());
                  setEditing(index);
                }}


                onClick={() => {
                  if (!onSeek) return;
                  if (window.getSelection()?.toString()) return;
                  onSeek(segment.startMs);
                }}
                className={cn(
                  "flex gap-3 rounded-md px-1.5 py-1 leading-relaxed transition-colors duration-fast ease-out",
                  onSeek && "cursor-pointer hover:bg-hover/60",
                  speaking && "bg-accent-subtle",
                )}
              >
                <span
                  data-numeric
                  className={cn(
                    "w-12 shrink-0 pt-px text-2xs tabular-nums",
                    speaking ? "text-accent" : "text-faint",
                  )}
                >
                  {clock(segment.startMs)}
                </span>
                <span className="min-w-0 flex-1 text-fg">
                  <Marked text={segment.text} needle={highlight} />
                </span>
              </p>
              </div>
            );
          })}
        </div>
      </ContextMenu>
    </div>
  );
}


function SpeakerName({
  name,
  editing,
  draft,
  onDraft,
  onStart,
  onSave,
  onCancel,
}: {
  name: string;
  editing: boolean;
  draft: string;
  onDraft: (value: string) => void;
  onStart: (() => void) | null;
  onSave: () => Promise<void>;
  onCancel: () => void;
}) {
  if (editing) {
    return (
      <div className="mt-3 mb-1 flex items-center gap-2 pl-1.5">
        <Input
          autoFocus
          value={draft}
          onChange={(event) => onDraft(event.target.value)}
          onBlur={() => void onSave()}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              void onSave();
            }
            if (event.key === "Escape") {
              event.preventDefault();
              onCancel();
            }
          }}
          aria-label={`Rename ${name}`}
          className="h-7 w-48 text-xs"
        />
      </div>
    );
  }

  return (
    <p className="mt-3 mb-1 pl-1.5 first:mt-0">
      <button
        type="button"
        onClick={() => onStart?.()}
        disabled={!onStart}
        title={onStart ? "Rename this speaker everywhere they speak" : undefined}
        className={cn(
          "inline-flex items-center gap-1.5 rounded-md px-1.5 py-0.5",
          "text-2xs font-medium tracking-wide text-accent uppercase",
          onStart && "hover:bg-accent-subtle [&_svg]:size-2.5 [&_svg]:opacity-0 hover:[&_svg]:opacity-100",
        )}
      >
        {name}
        {onStart ? <Pencil /> : null}
      </button>
    </p>
  );
}


function Marked({ text, needle }: { text: string; needle: string }) {
  const wanted = needle.trim().toLowerCase();
  if (!wanted) return <>{text}</>;

  const pieces: React.ReactNode[] = [];
  const haystack = text.toLowerCase();
  let at = 0;

  for (;;) {
    const found = haystack.indexOf(wanted, at);
    if (found < 0) break;

    if (found > at) pieces.push(text.slice(at, found));
    pieces.push(
      <mark key={found} className="rounded-xs bg-warning-subtle px-0.5 text-fg">
        {text.slice(found, found + wanted.length)}
      </mark>,
    );
    at = found + wanted.length;
  }

  if (at === 0) return <>{text}</>;
  if (at < text.length) pieces.push(text.slice(at));
  return <>{pieces}</>;
}
