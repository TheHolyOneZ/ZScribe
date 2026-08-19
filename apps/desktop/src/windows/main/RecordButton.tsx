import { useState } from "react";
import { Mic, Square } from "lucide-react";

import { Callout } from "@/components/ui";
import { cn } from "@/lib/cn";
import { clock } from "@/lib/format";
import { ipc, toCommandError, type CommandError } from "@/lib/ipc";
import { useAppStore } from "@/store/useAppStore";


export function RecordButton() {
  const status = useAppStore((state) => state.status);
  const [error, setError] = useState<CommandError | null>(null);

  const toggle = async () => {
    setError(null);
    try {
      if (status.active) {
        await ipc.stopRecording();
      } else {
        await ipc.startRecording();
      }
    } catch (raw) {
      setError(toCommandError(raw));
    }
  };

  return (
    <>
      <button
        type="button"
        onClick={() => void toggle()}
        aria-label={status.active ? "Stop recording" : "Start recording"}
        className={cn(
          "inline-flex h-7 items-center gap-2 rounded-md px-2.5",
          "text-xs font-medium transition-colors duration-fast ease-out",
          "[&_svg]:size-3.5",
          status.active
            ? "bg-danger text-white hover:bg-danger-hover"
            : "text-muted hover:bg-hover hover:text-fg",
        )}
      >
        {status.active ? <Square /> : <Mic />}
        {status.active ? (
          <span data-numeric>{clock(status.durationMs)}</span>
        ) : (
          <span>Record</span>
        )}
      </button>

      {error ? (
        <div className="fixed inset-x-0 top-12 z-50 mx-auto w-full max-w-md px-4">
          <Callout
            tone="danger"
            title={error.message}
            action={
              <button
                type="button"
                onClick={() => setError(null)}
                className="text-xs text-muted hover:text-fg"
              >
                Dismiss
              </button>
            }
          >
            {error.remedy}
          </Callout>
        </div>
      ) : null}
    </>
  );
}
