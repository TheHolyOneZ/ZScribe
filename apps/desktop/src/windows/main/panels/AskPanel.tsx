import { useCallback, useEffect, useRef, useState } from "react";
import {
  CornerDownLeft,
  Database,
  Headphones,
  Loader2,
  Search,
  Sparkles,
} from "lucide-react";

import { Button, Callout, EmptyState, Panel, Skeleton, Textarea } from "@/components/ui";
import { Copyable } from "@/components/Copyable";
import { Markdown } from "@/components/Markdown";
import { cn } from "@/lib/cn";
import { clock, timestamp } from "@/lib/format";
import {
  EVENTS,
  ipc,
  on,
  toCommandError,
  type ArchiveAnswer,
  type ArchiveStatus,
  type CommandError,
  type IndexProgress,
} from "@/lib/ipc";


export function AskPanel() {
  const [status, setStatus] = useState<ArchiveStatus | null>(null);
  const [indexing, setIndexing] = useState<IndexProgress | null>(null);
  const [question, setQuestion] = useState("");
  const [asking, setAsking] = useState(false);
  const [answer, setAnswer] = useState<ArchiveAnswer | null>(null);
  const [failure, setFailure] = useState<CommandError | null>(null);

  const box = useRef<HTMLTextAreaElement>(null);

  const refresh = useCallback(async () => {
    try {
      setStatus(await ipc.archiveStatus());
    } catch (raw) {
      setFailure(toCommandError(raw));
    }
  }, []);

  useEffect(() => {
    void refresh();

    const listener = on<IndexProgress>(EVENTS.indexProgress, (progress) => {


      setIndexing(progress.done >= progress.total ? null : progress);
    });

    return () => {
      void listener.then((off) => off());
    };
  }, [refresh]);

  const index = async () => {
    setFailure(null);
    setIndexing({ done: 0, total: status?.transcribed ?? 0, title: "" });
    try {
      await ipc.indexArchive();
    } catch (raw) {
      setFailure(toCommandError(raw));
    } finally {
      setIndexing(null);
      await refresh();
    }
  };

  const ask = async () => {
    const asked = question.trim();
    if (!asked || asking) return;

    setAsking(true);
    setFailure(null);
    setAnswer(null);
    try {
      setAnswer(await ipc.askArchive(asked));
    } catch (raw) {
      setFailure(toCommandError(raw));
    } finally {
      setAsking(false);
    }
  };

  const waiting = status ? status.transcribed - status.indexed : 0;
  const ready = status?.ollamaReady === true && status.modelReady && status.indexed > 0;

  return (
    <Panel
      title="Ask your recordings"
      description="One question, answered from every transcript you have — with the moments it came from."
    >
      {!status ? (
        <div className="space-y-3">
          <Skeleton className="h-16 w-full" />
          <Skeleton className="h-24 w-full" />
        </div>
      ) : (
        <div className="space-y-5">
          <IndexState
            status={status}
            waiting={waiting}
            indexing={indexing}
            onIndex={() => void index()}
          />

          <div>
            <div className="flex items-end gap-2">
              <Textarea
                ref={box}
                value={question}
                onChange={(event) => setQuestion(event.target.value)}
                onKeyDown={(event) => {


                  if (event.key === "Enter" && !event.shiftKey) {
                    event.preventDefault();
                    void ask();
                  }
                }}
                rows={2}
                disabled={!ready || asking}
                placeholder="What did we decide about the pricing?"
                aria-label="Ask a question about all your recordings"
                className="flex-1"
              />
              <Button
                variant="primary"
                disabled={!ready || asking || !question.trim()}
                onClick={() => void ask()}
              >
                {asking ? <Loader2 className="animate-spin" /> : <CornerDownLeft />}
                Ask
              </Button>
            </div>

            {ready ? (
              <p className="mt-1.5 text-2xs leading-relaxed text-faint">
                Answers come only from your recordings. When they do not cover something, it says
                so rather than inventing it.
              </p>
            ) : null}
          </div>

          {failure ? (
            <Callout tone="danger" title={failure.message}>
              {failure.remedy}
            </Callout>
          ) : null}

          {asking ? (
            <div className="space-y-2">
              <Skeleton className="h-3 w-1/3" />
              <Skeleton className="h-16 w-full" />
            </div>
          ) : answer ? (
            <Answer answer={answer} />
          ) : ready ? (
            <EmptyState
              icon={<Sparkles />}
              title="Nothing asked yet"
              description="Try a question you would otherwise have to remember the meeting for."
            />
          ) : null}
        </div>
      )}
    </Panel>
  );
}


function IndexState({
  status,
  waiting,
  indexing,
  onIndex,
}: {
  status: ArchiveStatus;
  waiting: number;
  indexing: IndexProgress | null;
  onIndex: () => void;
}) {
  if (!status.ollamaReady) {
    return (
      <Callout tone="neutral" title="This needs Ollama">
        Questions across recordings are answered from vectors built on this machine, and Ollama is
        what builds them. It is the same Ollama the summaries use — start it, and this page starts
        working.
      </Callout>
    );
  }

  if (!status.modelReady) {
    return (
      <Callout tone="neutral" title={`The embedding model is not installed`}>
        <p>
          Passages and questions have to be turned into vectors by the same model.{" "}
          <span className="font-mono text-fg">{status.embeddingModel}</span> is small — about 270
          MB — and runs on the processor.
        </p>


        <span className="mt-2 block">
          <Copyable text={`ollama pull ${status.embeddingModel}`} />
        </span>
      </Callout>
    );
  }

  return (
    <div className="rounded-lg border border-line-subtle bg-surface px-3.5 py-3">
      <div className="flex items-center justify-between gap-4">
        <div className="min-w-0">
          <p className="flex items-center gap-2 text-sm text-fg [&_svg]:size-3.5 [&_svg]:text-faint">
            <Database />
            {status.indexed} of {status.transcribed} recordings indexed
          </p>
          <p data-numeric className="mt-0.5 text-2xs text-faint">
            {status.passages} passages · {status.embeddingModel}
          </p>
        </div>

        <Button
          size="sm"
          variant={waiting > 0 ? "primary" : "secondary"}
          disabled={indexing !== null || waiting === 0}
          onClick={onIndex}
        >
          {indexing ? <Loader2 className="animate-spin" /> : <Search />}
          {indexing
            ? "Indexing…"
            : waiting > 0
              ? `Index ${waiting} more`
              : "Everything is indexed"}
        </Button>
      </div>

      {indexing ? (
        <div className="mt-2.5 space-y-1">
          <div className="h-1 overflow-hidden rounded-full bg-inset">
            <div
              className="h-full rounded-full bg-accent transition-[width] duration-base ease-out"
              style={{
                width: `${indexing.total > 0 ? (indexing.done / indexing.total) * 100 : 0}%`,
              }}
            />
          </div>
          <p data-numeric className="truncate text-2xs text-faint">
            {indexing.done} of {indexing.total}
            {indexing.title ? ` · ${indexing.title}` : ""}
          </p>
        </div>
      ) : null}
    </div>
  );
}

function Answer({ answer }: { answer: ArchiveAnswer }) {
  return (
    <div className="space-y-4">
      <Markdown source={answer.text} />

      {answer.citations.length > 0 ? (
        <section>
          <h2 className="mb-2 text-xs font-medium tracking-wide text-muted uppercase">
            From these moments
          </h2>

          <ul className="space-y-1.5">
            {answer.citations.map((citation, index) => (
              <li key={index}>


                <button
                  type="button"
                  onClick={() => void ipc.openPlayer(citation.recordingId, citation.startMs)}
                  className={cn(
                    "block w-full rounded-lg border border-line-subtle bg-surface px-3 py-2 text-left",
                    "transition-colors duration-fast ease-out hover:border-line hover:bg-hover/50",
                  )}
                >
                  <p className="flex items-center gap-2 text-xs text-fg [&_svg]:size-3 [&_svg]:text-faint">
                    <Headphones />
                    <span className="truncate">{citation.title}</span>
                    <span data-numeric className="shrink-0 text-2xs text-faint">
                      {clock(citation.startMs)} · {timestamp(citation.startedAt)}
                    </span>
                  </p>
                  <p className="mt-1 line-clamp-2 text-2xs leading-relaxed text-muted">
                    {citation.text}
                  </p>
                </button>
              </li>
            ))}
          </ul>
        </section>
      ) : null}
    </div>
  );
}
