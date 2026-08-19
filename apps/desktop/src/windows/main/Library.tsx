import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import {
  Captions,
  Check,
  Copy,
  ChevronDown,
  Download,
  FileText,
  FolderOpen,
  Headphones,
  MessageSquare,
  Mic,
  Pencil,
  RefreshCw,
  Sparkles,
  Trash2,
} from "lucide-react";

import {
  Button,
  Callout,
  ContextMenu,
  Dialog,
  DropdownItem,
  DropdownMenu,
  EmptyState,
  Input,
  MenuItem,
  MenuLabel,
  MenuSeparator,
  Skeleton,
  StatusDot,
} from "@/components/ui";
import { Chat } from "./Chat";
import { Markdown } from "@/components/Markdown";
import { TagFilter, Tags } from "./Tags";
import { TranscriptView } from "@/components/TranscriptView";
import { CopySelection, copyText, useSelection } from "@/components/selection";
import { Player, type PlayerHandle } from "@/components/Player";
import { cn } from "@/lib/cn";
import { useEta } from "@/lib/eta";
import { clock, relativeTime, timestamp } from "@/lib/format";
import { lineAt } from "@/lib/transcript";
import {
  EVENTS,
  ipc,
  on,
  toCommandError,
  type CommandError,
  type PipelineFailure,
  type Recording,
  type RecordingDetail,
  type SearchHit,
  SLOWDOWN_NOTE,
  stageLabel,
} from "@/lib/ipc";
import { useAppStore } from "@/store/useAppStore";

type Tab = "summary" | "transcript" | "chat";


type Request = { id: string; action: "rename" | "summary" | "transcript" };

export function Library() {
  const recordings = useAppStore((state) => state.recordings);
  const selectedId = useAppStore((state) => state.selectedId);
  const select = useAppStore((state) => state.select);
  const loading = useAppStore((state) => state.loading);
  const refreshRecordings = useAppStore((state) => state.refreshRecordings);

  const [request, setRequest] = useState<Request | null>(null);
  const [pendingDelete, setPendingDelete] = useState<Recording | null>(null);

  const ask = (recording: Recording, action: Request["action"]) => {
    select(recording.id);
    setRequest({ id: recording.id, action });
  };

  const remove = async () => {
    if (!pendingDelete) return;
    setPendingDelete(null);
    await ipc.deleteRecording(pendingDelete.id);
    await refreshRecordings();
  };

  if (!loading && recordings.length === 0) {
    return (
      <div className="flex h-full items-center justify-center">
        <EmptyState
          icon={<Mic />}
          title="No recordings yet"
          description="Press the hotkey, or the Record button in the title bar. Import brings in a file or a link you already have. Everything stays on this machine."
        />
      </div>
    );
  }

  return (
    <div className="flex h-full min-w-0">
      <RecordingList
        recordings={recordings}
        selectedId={selectedId}
        onSelect={select}
        onAsk={ask}
        onDelete={setPendingDelete}
      />
      <div className="min-w-0 flex-1">
        {selectedId ? (
          <Detail
            id={selectedId}
            request={request}
            onRequestHandled={() => setRequest(null)}
            onDelete={setPendingDelete}
          />
        ) : null}
      </div>


      <Dialog
        open={pendingDelete !== null}
        onOpenChange={(open) => !open && setPendingDelete(null)}
        title={`Delete “${pendingDelete?.title ?? ""}”?`}
        width="sm"
        footer={
          <>
            <Button variant="ghost" onClick={() => setPendingDelete(null)}>
              Cancel
            </Button>
            <Button variant="danger" onClick={() => void remove()}>
              Delete
            </Button>
          </>
        }
      >
        <p className="text-sm leading-relaxed text-muted">
          The recording, its transcript and its summary are removed together. There is no undo and
          no copy anywhere else.
        </p>
      </Dialog>
    </div>
  );
}


const SEARCH_LIMIT = 200;


const MARK_OPEN = "\ue000";
const MARK_CLOSE = "\ue001";

function Snippet({ text }: { text: string }) {


  const pieces = text.split(new RegExp(`[${MARK_OPEN}${MARK_CLOSE}]`));

  return (
    <>
      {pieces.map((piece, index) =>
        index % 2 === 1 ? (
          <mark key={index} className="rounded-xs bg-accent-subtle px-0.5 text-fg">
            {piece}
          </mark>
        ) : (
          <span key={index}>{piece}</span>
        ),
      )}
    </>
  );
}


async function copyMarkdown(id: string) {
  await navigator.clipboard.writeText(await ipc.recordingMarkdown(id));
}


const asFilename = (title: string) => title.replace(/[/\\:*?"<>|]/g, "-");

async function exportMarkdown(recording: Recording) {
  const path = await save({
    defaultPath: `${asFilename(recording.title)}.md`,
    filters: [{ name: "Markdown", extensions: ["md"] }],
  });
  if (!path) return;

  await ipc.exportRecording(recording.id, path, "markdown");
}


async function exportSubtitles(recording: Recording, vtt: boolean) {
  const extension = vtt ? "vtt" : "srt";
  const path = await save({
    defaultPath: `${asFilename(recording.title)}.${extension}`,
    filters: [{ name: vtt ? "WebVTT" : "SubRip", extensions: [extension] }],
  });
  if (!path) return;

  await ipc.exportRecording(recording.id, path, vtt ? "vtt" : "srt");
}


function RecordingMenu({
  recording,
  onAsk,
  onDelete,
}: {
  recording: Recording;
  onAsk: (recording: Recording, action: Request["action"]) => void;
  onDelete: (recording: Recording) => void;
}) {
  return (
    <>
      <MenuLabel>{recording.title}</MenuLabel>

      <MenuItem icon={<Pencil />} onSelect={() => onAsk(recording, "rename")}>
        Rename…
      </MenuItem>

      <MenuSeparator />

      <MenuItem
        icon={<Headphones />}
        disabled={!recording.hasTranscript}
        onSelect={() => void ipc.openPlayer(recording.id)}
      >
        Open the player
      </MenuItem>

      <MenuSeparator />

      <MenuItem icon={<Copy />} onSelect={() => void copyMarkdown(recording.id)}>
        Copy as Markdown
      </MenuItem>
      <MenuItem icon={<Download />} onSelect={() => void exportMarkdown(recording)}>
        Export as Markdown…
      </MenuItem>
      <MenuItem
        icon={<Captions />}
        disabled={!recording.hasTranscript}
        onSelect={() => void exportSubtitles(recording, false)}
      >
        Export subtitles (SRT)…
      </MenuItem>
      <MenuItem
        icon={<Captions />}
        disabled={!recording.hasTranscript}
        onSelect={() => void exportSubtitles(recording, true)}
      >
        Export subtitles (VTT)…
      </MenuItem>
      <MenuItem
        icon={<FolderOpen />}
        disabled={!recording.audioPath}
        onSelect={() => recording.audioPath && void revealItemInDir(recording.audioPath)}
      >
        Show the audio file
      </MenuItem>

      <MenuSeparator />

      <MenuItem
        icon={<Sparkles />}
        disabled={!recording.hasTranscript}
        onSelect={() => onAsk(recording, "summary")}
      >
        Summarise again
      </MenuItem>
      <MenuItem
        icon={<FileText />}
        disabled={!recording.audioPath}
        onSelect={() => onAsk(recording, "transcript")}
      >
        Transcribe again
      </MenuItem>

      <MenuSeparator />

      <MenuItem icon={<Trash2 />} tone="danger" onSelect={() => onDelete(recording)}>
        Delete…
      </MenuItem>
    </>
  );
}

function RecordingList({
  recordings,
  selectedId,
  onSelect,
  onAsk,
  onDelete,
}: {
  recordings: Recording[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onAsk: (recording: Recording, action: Request["action"]) => void;
  onDelete: (recording: Recording) => void;
}) {
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<SearchHit[] | null>(null);
  const [tags, setTags] = useState<[string, number][]>([]);
  const [tag, setTag] = useState<string | null>(null);


  useEffect(() => {
    void ipc
      .listTags()
      .then(setTags)
      .catch(() => setTags([]));
  }, [recordings]);


  useEffect(() => {
    const needle = query.trim();
    if (!needle) {
      setHits(null);
      return;
    }


    const timer = setTimeout(() => {
      void ipc
        .searchRecordings(needle, SEARCH_LIMIT)
        .then(setHits)
        .catch(() => setHits([]));
    }, 120);

    return () => clearTimeout(timer);
  }, [query]);

  const shown = useMemo(() => {
    const filed = (recording: Recording) => tag === null || recording.tags.includes(tag);

    if (hits === null) {
      return recordings.filter(filed).map((recording) => ({ recording, snippet: null }));
    }


    const found = new Set(hits.map((hit) => hit.recording.id));
    const needle = query.trim().toLowerCase();

    return [
      ...hits
        .filter((hit) => filed(hit.recording))
        .map((hit) => ({ recording: hit.recording, snippet: hit.snippet })),
      ...recordings
        .filter(
          (recording) =>
            !found.has(recording.id) &&
            filed(recording) &&
            recording.title.toLowerCase().includes(needle),
        )
        .map((recording) => ({ recording, snippet: null })),
    ];
  }, [hits, query, recordings, tag]);

  return (
    <aside className="flex w-72 shrink-0 flex-col border-r border-line-subtle">
      <div className="border-b border-line-subtle p-2.5">
        <Input
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Escape") setQuery("");
          }}
          placeholder="Search everything"
          aria-label="Search recordings, transcripts and summaries"
          spellCheck={false}
        />
      </div>

      <TagFilter tags={tags} active={tag} onPick={setTag} />

      <div className="scroll-area min-h-0 flex-1">
        {shown.length === 0 ? (
          <p className="px-4 py-6 text-center text-xs leading-relaxed text-faint">
            Nothing matches that — not in a title, a summary, or anything said.
          </p>
        ) : (
          shown.map(({ recording, snippet }) => (
            <ContextMenu
              key={recording.id}
              content={
                <RecordingMenu recording={recording} onAsk={onAsk} onDelete={onDelete} />
              }
            >
              <button


                onContextMenu={() => onSelect(recording.id)}
                onClick={() => onSelect(recording.id)}
                aria-pressed={recording.id === selectedId}
                className={cn(
                  "block w-full border-b border-line-subtle px-3.5 py-2.5 text-left",
                  "transition-colors duration-fast ease-out",
                  recording.id === selectedId ? "bg-accent-subtle" : "hover:bg-hover/50",
                )}
              >
                <p className="truncate text-sm text-fg">{recording.title}</p>

                {snippet ? (
                  <p className="mt-1 line-clamp-2 text-2xs leading-relaxed text-muted">
                    <Snippet text={snippet} />
                  </p>
                ) : null}

                <p data-numeric className="mt-0.5 flex items-center gap-2 text-2xs text-faint">
                  <span>{relativeTime(recording.startedAt)}</span>
                  <span aria-hidden>·</span>
                  <span>{clock(recording.durationMs)}</span>
                  {!recording.hasTranscript ? (
                    <>
                      <span aria-hidden>·</span>
                      <span className="text-warning">no transcript</span>
                    </>
                  ) : !recording.hasSummary ? (
                    <>
                      <span aria-hidden>·</span>
                      <span className="text-muted">no summary</span>
                    </>
                  ) : null}
                </p>
              </button>
            </ContextMenu>
          ))
        )}
      </div>
    </aside>
  );
}

function Detail({
  id,
  request,
  onRequestHandled,
  onDelete,
}: {
  id: string;
  request: Request | null;
  onRequestHandled: () => void;
  onDelete: (recording: Recording) => void;
}) {
  const [detail, setDetail] = useState<RecordingDetail | null>(null);
  const [tab, setTab] = useState<Tab>("summary");
  const [renaming, setRenaming] = useState(false);
  const [draft, setDraft] = useState("");
  const [copied, setCopied] = useState(false);
  const [failure, setFailure] = useState<CommandError | null>(null);
  const [busy, setBusy] = useState(false);
  const nameInput = useRef<HTMLInputElement>(null);


  useEffect(() => {
    if (!renaming) return;

    const focus = () => {
      nameInput.current?.focus();
      nameInput.current?.select();
    };

    focus();
    const timer = setTimeout(focus, 60);
    return () => clearTimeout(timer);
  }, [renaming]);

  const progress = useAppStore((state) => state.progress);
  const refreshRecordings = useAppStore((state) => state.refreshRecordings);


  const eta = useEta(
    progress?.recordingId === id ? (progress?.percent ?? null) : null,
    progress ? `${progress.stage}:${progress.recordingId}` : "",
  );

  const load = useCallback(async () => {
    setDetail(await ipc.getRecording(id));
  }, [id]);

  useEffect(() => {
    setDetail(null);
    setFailure(null);
    setSummaryIsStale(false);
    void load();
  }, [id, load]);

  useEffect(() => {
    const listeners = [
      on<string>(EVENTS.recordingReady, (readyId) => {
        if (readyId === id) void load();
      }),
      on<PipelineFailure>(EVENTS.pipelineFailed, (event) => {
        if (event.recordingId === id) setFailure(event.error);
      }),
    ];
    return () => {
      for (const listener of listeners) void listener.then((off) => off());
    };
  }, [id, load]);


  const [spokenIndex, setSpokenIndex] = useState(-1);
  const player = useRef<PlayerHandle>(null);
  const segments = detail?.transcript?.segments;

  const followAlong = useCallback(
    (ms: number) => {
      setSpokenIndex(segments ? lineAt(segments, ms) : -1);
    },
    [segments],
  );


  const [summaryIsStale, setSummaryIsStale] = useState(false);

  const correctLine = useCallback(
    async (index: number, text: string) => {
      try {
        await ipc.editTranscriptLine(id, index, text);
        setSummaryIsStale(true);
        await load();
      } catch (raw) {
        setFailure(toCommandError(raw));
      }
    },
    [id, load],
  );

  const renameSpeaker = useCallback(
    async (from: string, to: string) => {
      try {
        await ipc.renameSpeaker(id, from, to);
        await load();
      } catch (raw) {
        setFailure(toCommandError(raw));
      }
    },
    [id, load],
  );

  const rerun = useCallback(
    async (what: "summary" | "transcript") => {
      setBusy(true);
      setFailure(null);
      try {
        if (what === "summary") {
          setSummaryIsStale(false);
          await ipc.resummarise(id);
        } else await ipc.retranscribe(id);
      } catch (raw) {
        setFailure(toCommandError(raw));
      } finally {
        setBusy(false);
      }
    },
    [id],
  );


  useEffect(() => {
    if (!detail || request?.id !== id) return;

    if (request.action === "rename") {
      setDraft(detail.recording.title);
      setRenaming(true);
    } else {
      void rerun(request.action);
    }
    onRequestHandled();
  }, [detail, request, id, rerun, onRequestHandled]);

  if (!detail) {
    return (
      <div className="space-y-3 p-6">
        <Skeleton className="h-5 w-1/3" />
        <Skeleton className="h-3 w-1/4" />
        <Skeleton className="h-24 w-full" />
      </div>
    );
  }

  const { recording, transcript, summary } = detail;
  const working = progress?.recordingId === id;

  const copy = async () => {
    await copyMarkdown(id);
    setCopied(true);
    setTimeout(() => setCopied(false), 1600);
  };

  const rename = async () => {
    setRenaming(false);
    if (draft.trim() && draft !== recording.title) {
      await ipc.renameRecording(id, draft.trim());
      await Promise.all([load(), refreshRecordings()]);
    }
  };

  return (
    <div className="scroll-area h-full">
      <div className="mx-auto max-w-3xl px-8 py-7">
        <ContextMenu
          content={
            <RecordingMenu
              recording={recording}
              onAsk={(_, action) => {
                if (action === "rename") {
                  setDraft(recording.title);
                  setRenaming(true);
                } else {
                  void rerun(action);
                }
              }}
              onDelete={onDelete}
            />
          }
        >
          <header className="mb-6">
          <div className="flex items-start justify-between gap-4">
            {renaming ? (
              <Input
                ref={nameInput}
                value={draft}
                onChange={(event) => setDraft(event.target.value)}
                onBlur={() => void rename()}
                onKeyDown={(event) => {
                  if (event.key === "Enter") void rename();
                  if (event.key === "Escape") setRenaming(false);
                }}
                aria-label="Recording name"
                className="text-lg"
              />
            ) : (
              <button
                type="button"
                onClick={() => {
                  setDraft(recording.title);
                  setRenaming(true);
                }}


                className="group flex min-w-0 flex-1 items-center gap-2 text-left"
                aria-label="Rename this recording"
              >


                <h1 className="line-clamp-2 text-lg font-medium text-balance text-fg">
                  {recording.title}
                </h1>
                <Pencil className="size-3.5 shrink-0 text-faint opacity-0 transition-opacity duration-fast group-hover:opacity-100" />
              </button>
            )}

            <div className="flex shrink-0 items-center gap-1.5">
              <Button size="sm" onClick={() => void copy()}>
                {copied ? <Check /> : <Copy />}
                {copied ? "Copied" : "Copy"}
              </Button>


              <DropdownMenu
                trigger={
                  <Button size="sm">
                    <Download />
                    Export
                    <ChevronDown />
                  </Button>
                }
              >
                <DropdownItem
                  icon={<FileText />}
                  description="The summary and the transcript, in one document"
                  onSelect={() => void exportMarkdown(recording)}
                >
                  Markdown…
                </DropdownItem>
                <DropdownItem
                  icon={<Captions />}
                  disabled={!transcript}
                  description="Subtitles for a video editor or a player"
                  onSelect={() => void exportSubtitles(recording, false)}
                >
                  SubRip (.srt)…
                </DropdownItem>
                <DropdownItem
                  icon={<Captions />}
                  disabled={!transcript}
                  description="Subtitles for the web, with speaker names"
                  onSelect={() => void exportSubtitles(recording, true)}
                >
                  WebVTT (.vtt)…
                </DropdownItem>
              </DropdownMenu>


              <Button
                size="sm"
                disabled={!transcript}
                title={
                  transcript
                    ? "Open this recording in its own player window"
                    : "There is no transcript to read along with yet"
                }
                onClick={() => void ipc.openPlayer(id)}
              >
                <Headphones />
                Player
              </Button>
              <Button
                size="sm"
                variant="ghost"
                icon
                aria-label="Delete this recording"
                onClick={() => onDelete(recording)}
              >
                <Trash2 />
              </Button>
            </div>
          </div>

          <div className="mt-2.5">
            <Tags
              tags={recording.tags}
              onChange={async (next) => {
                await ipc.setTags(id, next);
                await Promise.all([load(), refreshRecordings()]);
              }}
            />
          </div>

          <p data-numeric className="mt-2 flex items-center gap-2 text-xs text-faint">
            <span>{timestamp(recording.startedAt)}</span>
            <span aria-hidden>·</span>
            <span>{clock(recording.durationMs)}</span>
            <span aria-hidden>·</span>
            <span>{recording.source}</span>
            {!recording.audioPath ? (
              <>
                <span aria-hidden>·</span>
                <span>audio deleted</span>
              </>
            ) : null}
          </p>
          </header>
        </ContextMenu>


        {recording.audioPath ? (
          <Player
            ref={player}
            recordingId={id}
            durationMs={recording.durationMs}
            onTime={followAlong}
          />
        ) : null}

        {working ? (
          <div className="mb-6 space-y-1.5">
            <StatusDot tone="accent">
              {stageLabel(progress)}
              {eta ? <span className="text-faint"> · {eta}</span> : null}
            </StatusDot>
            <div className="h-1 overflow-hidden rounded-full bg-inset">
              <div
                className="h-full rounded-full bg-accent transition-[width] duration-base ease-out"
                style={{ width: `${progress.percent}%` }}
              />
            </div>
            {progress.onThisMachine ? (
              <p className="text-xs leading-relaxed text-faint">{SLOWDOWN_NOTE}</p>
            ) : null}
          </div>
        ) : null}

        {failure ? (
          <Callout
            tone="warning"
            title={failure.message}
            className="mb-6"
            action={
              failure.retryable ? (
                <Button size="sm" onClick={() => void rerun("summary")} disabled={busy}>
                  Try again
                </Button>
              ) : null
            }
          >
            {failure.remedy}
          </Callout>
        ) : null}

        <div
          role="tablist"
          aria-label="Recording views"
          className="mb-4 flex items-center gap-1 border-b border-line-subtle"


          onKeyDown={(event) => {
            if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
            event.preventDefault();

            const order: Tab[] = ["summary", "transcript", "chat"];
            const step = event.key === "ArrowRight" ? 1 : order.length - 1;
            setTab(order[(order.indexOf(tab) + step) % order.length]!);
          }}
        >
          <TabButton
            active={tab === "summary"}
            onClick={() => setTab("summary")}
            icon={<Sparkles />}
          >
            Summary
          </TabButton>
          <TabButton
            active={tab === "transcript"}
            onClick={() => setTab("transcript")}
            icon={<FileText />}
          >
            Transcript
          </TabButton>
          <TabButton active={tab === "chat"} onClick={() => setTab("chat")} icon={<MessageSquare />}>
            Chat
          </TabButton>

          <div className="flex-1" />

          {tab === "chat" ? null : (
          <Button
            size="sm"
            variant="ghost"
            disabled={busy || working || (tab === "transcript" && !recording.audioPath)}
            onClick={() => void rerun(tab === "summary" ? "summary" : "transcript")}
            title={
              tab === "transcript" && !recording.audioPath
                ? "The audio for this recording has been deleted"
                : undefined
            }
          >
            <RefreshCw className={cn(busy && "animate-spin")} />
            {tab === "summary" ? "Summarise again" : "Transcribe again"}
          </Button>
          )}
        </div>

        {tab === "chat" ? (
          <div className="h-[26rem]">
            <Chat
              recordingId={id}
              hasTranscript={transcript !== null}
              model={summary?.model ?? "the configured model"}
            />
          </div>
        ) : tab === "summary" ? (
          summary ? (
            <>
              {summaryIsStale ? (
                <Callout
                  tone="neutral"
                  className="mb-4"
                  title="This summary is older than the transcript"
                  action={
                    <Button size="sm" onClick={() => void rerun("summary")} disabled={busy}>
                      Summarise again
                    </Button>
                  }
                >
                  It was written before you corrected the transcript, so it still has the old
                  wording in it.
                </Callout>
              ) : null}
              <SummaryView summary={summary} />
            </>
          ) : (
            <EmptyState
              title={working ? "Working on it" : "No summary yet"}
              description={
                working
                  ? "The transcript is saved already — the summary is being written now."
                  : transcript
                    ? "The transcript is saved. Press “Summarise again” to write one."
                    : "A summary needs a transcript first."
              }
            />
          )
        ) : transcript ? (
          <TranscriptView
            transcript={transcript}
            spokenIndex={spokenIndex}
            onSeek={recording.audioPath ? (ms) => player.current?.playFrom(ms) : null}
            onEdit={correctLine}
            onRenameSpeaker={renameSpeaker}
          />
        ) : (
          <EmptyState
            title={working ? "Transcribing" : "No transcript"}
            description={
              working
                ? "Whisper is working through the audio."
                : "Transcription did not finish. If the audio is still here, try again."
            }
          />
        )}
      </div>
    </div>
  );
}

function TabButton({
  active,
  onClick,
  icon,
  children,
}: {
  active: boolean;
  onClick: () => void;
  icon: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      role="tab"
      aria-selected={active}


      tabIndex={active ? 0 : -1}
      onClick={onClick}
      className={cn(
        "inline-flex items-center gap-1.5 border-b-2 px-3 py-2 text-sm",
        "transition-colors duration-fast ease-out [&_svg]:size-3.5",
        active
          ? "border-accent text-fg"
          : "border-transparent text-muted hover:text-fg",
      )}
    >
      {icon}
      {children}
    </button>
  );
}

function SummaryView({ summary }: { summary: NonNullable<RecordingDetail["summary"]> }) {
  const { selection, capture } = useSelection();

  const actions = summary.actionItems
    .map((item) =>
      [item.task, item.owner, item.due].filter(Boolean).join(" — "),
    )
    .join("\n");

  return (
    <ContextMenu
      content={
        <>
          <CopySelection selection={selection} />
          <MenuItem icon={<Sparkles />} onSelect={() => copyText(summary.bodyMd)}>
            Copy the summary
          </MenuItem>
          <MenuItem
            icon={<Check />}
            disabled={summary.actionItems.length === 0}
            onSelect={() => copyText(actions)}
          >
            Copy the action items
          </MenuItem>
        </>
      }
    >
    <div className="space-y-5" onContextMenu={capture}>
      {summary.actionItems.length > 0 ? (
        <section>
          <h2 className="mb-2 text-xs font-medium tracking-wide text-muted uppercase">
            Action items
          </h2>
          <ul className="divide-y divide-line-subtle overflow-hidden rounded-lg border border-line-subtle bg-surface">
            {summary.actionItems.map((item, index) => (
              <li key={index} className="flex items-baseline gap-3 px-3.5 py-2.5">
                <span className="min-w-0 flex-1 text-sm text-fg">{item.task}</span>
                {item.owner ? (
                  <span className="shrink-0 text-xs text-muted">{item.owner}</span>
                ) : null}
                {item.due ? (
                  <span data-numeric className="shrink-0 text-xs text-faint">
                    {item.due}
                  </span>
                ) : null}
              </li>
            ))}
          </ul>
        </section>
      ) : null}

      <Markdown source={summary.bodyMd} />

      <p data-numeric className="text-2xs text-faint">
        {summary.model} · {summary.usage.input + summary.usage.output} tokens ·{" "}
        {(summary.elapsedMs / 1000).toFixed(1)}s


        {summary.redacted > 0 ? (
          <>
            {" · "}
            <span title="Contact details and names were replaced before the transcript was sent. Your copy is untouched.">
              {summary.redacted} detail{summary.redacted === 1 ? "" : "s"} withheld
            </span>
          </>
        ) : null}
      </p>
    </div>
    </ContextMenu>
  );
}
