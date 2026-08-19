import { useEffect, useRef, useState } from "react";
import { Copy, CornerDownLeft, Loader2, MessagesSquare, Sparkles, Trash2 } from "lucide-react";

import {
  Button,
  Callout,
  ContextMenu,
  EmptyState,
  MenuItem,
  MenuSeparator,
  Textarea,
} from "@/components/ui";
import { Markdown } from "@/components/Markdown";
import { cn } from "@/lib/cn";
import { ipc, toCommandError, type CommandError, type Turn } from "@/lib/ipc";


export function Chat({
  recordingId,
  hasTranscript,
  model,
}: {
  recordingId: string;
  hasTranscript: boolean;
  model: string;
}) {
  const [turns, setTurns] = useState<Turn[]>([]);
  const [question, setQuestion] = useState("");
  const [asking, setAsking] = useState(false);
  const [error, setError] = useState<CommandError | null>(null);

  const scroller = useRef<HTMLDivElement>(null);


  useEffect(() => {
    setTurns([]);
    setQuestion("");
    setError(null);
  }, [recordingId]);

  useEffect(() => {
    const element = scroller.current;
    if (element) element.scrollTop = element.scrollHeight;
  }, [turns, asking]);

  const ask = async () => {
    const asked = question.trim();
    if (!asked || asking) return;

    setError(null);
    setAsking(true);
    setQuestion("");


    const history = turns;
    setTurns([...history, { role: "user", content: asked }]);

    try {
      const answer = await ipc.ask(recordingId, history, asked);
      setTurns([...history, { role: "user", content: asked }, { role: "assistant", content: answer }]);
    } catch (raw) {
      setError(toCommandError(raw));

      setTurns(history);
      setQuestion(asked);
    } finally {
      setAsking(false);
    }
  };

  if (!hasTranscript) {
    return (
      <EmptyState
        title="Nothing to ask about yet"
        description="A question needs a transcript. Once this recording has one, you can ask about it here."
      />
    );
  }

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div ref={scroller} className="scroll-area min-h-0 flex-1 pb-3">
        {turns.length === 0 ? (
          <EmptyState
            icon={<Sparkles />}
            title="Ask about this recording"
            description="What was decided, who said what, or “turn the next steps into an email”. It answers from the transcript, and says so when the transcript does not cover it."
          />
        ) : (
          <div className="space-y-4">
            {turns.map((turn, index) => (
              <Message key={index} turn={turn} onCopyAll={() => copyConversation(turns)} onClear={() => setTurns([])} />
            ))}

            {asking ? (
              <p className="inline-flex items-center gap-2 text-xs text-faint [&_svg]:size-3.5">
                <Loader2 className="animate-spin" />
                Reading the transcript…
              </p>
            ) : null}
          </div>
        )}
      </div>

      {error ? (
        <Callout tone="warning" title={error.message} className="mb-3">
          {error.remedy}
        </Callout>
      ) : null}

      <div className="flex items-end gap-2 border-t border-line-subtle pt-3">
        <Textarea
          value={question}
          onChange={(event) => setQuestion(event.target.value)}
          onKeyDown={(event) => {

            if (event.key === "Enter" && !event.shiftKey) {
              event.preventDefault();
              void ask();
            }
          }}
          placeholder="What was agreed?"
          rows={2}
          aria-label="Ask about this recording"
          className="flex-1 resize-none"
        />
        <Button
          variant="primary"
          disabled={!question.trim() || asking}
          onClick={() => void ask()}
          aria-label="Ask"
        >
          <CornerDownLeft />
          Ask
        </Button>
      </div>

      <p data-numeric className="mt-2 text-2xs text-faint">
        {model} · answers come from the transcript, not from the model's own knowledge
      </p>
    </div>
  );
}

/** The conversation as plain text, in the order it happened. */
function copyConversation(turns: Turn[]) {
  const text = turns
    .map((turn) => `${turn.role === "user" ? "Q" : "A"}: ${turn.content.trim()}`)
    .join("\n\n");
  void navigator.clipboard.writeText(text);
}

function Message({
  turn,
  onCopyAll,
  onClear,
}: {
  turn: Turn;
  onCopyAll: () => void;
  onClear: () => void;
}) {
  const mine = turn.role === "user";
  const [selection, setSelection] = useState("");

  return (
    <div className={cn("flex", mine && "justify-end")}>
      <ContextMenu
        content={
          <>
            {selection ? (
              <>
                <MenuItem icon={<Copy />} onSelect={() => void navigator.clipboard.writeText(selection)}>
                  Copy selection
                </MenuItem>
                <MenuSeparator />
              </>
            ) : null}
            <MenuItem icon={<Copy />} onSelect={() => void navigator.clipboard.writeText(turn.content)}>
              {mine ? "Copy this question" : "Copy this answer"}
            </MenuItem>
            <MenuItem icon={<MessagesSquare />} onSelect={onCopyAll}>
              Copy the conversation
            </MenuItem>
            <MenuSeparator />
            {/* The conversation is never saved, so clearing it costs nothing
                and there is nothing to confirm. */}
            <MenuItem icon={<Trash2 />} tone="danger" onSelect={onClear}>
              Clear the conversation
            </MenuItem>
          </>
        }
      >
        <div
          data-selectable
          onContextMenu={() => setSelection(window.getSelection()?.toString().trim() ?? "")}
          className={cn(
            "max-w-[85%] rounded-lg px-3 py-2 text-sm leading-relaxed",
            mine ? "bg-accent-subtle text-fg whitespace-pre-wrap" : "bg-raised text-fg",
          )}
        >
          {/* The question is what the user typed, and it should come back
              looking exactly like it — nobody writes Markdown at a chat box on
              purpose. The answer is a model's, and models write Markdown
              whether or not anyone asked. */}
          {mine ? turn.content : <Markdown source={turn.content} />}
        </div>
      </ContextMenu>
    </div>
  );
}
