import { useState } from "react";
import { Check, Copy } from "lucide-react";

import { cn } from "@/lib/cn";


export function Copyable({ text }: { text: string }) {
  const [copied, setCopied] = useState(false);

  return (
    <button
      type="button"
      onClick={() => {
        void navigator.clipboard.writeText(text);
        setCopied(true);
        setTimeout(() => setCopied(false), 1600);
      }}
      title="Copy"
      className={cn(
        "flex w-full items-center gap-2 rounded-md border border-line-subtle bg-inset px-2.5 py-1.5",
        "text-left font-mono text-2xs text-fg",
        "transition-colors duration-fast ease-out hover:border-line hover:bg-hover/50",
        "[&_svg]:size-3 [&_svg]:shrink-0 [&_svg]:text-faint",
      )}
    >
      <span data-selectable className="min-w-0 flex-1 truncate">
        {text}
      </span>
      {copied ? <Check /> : <Copy />}
    </button>
  );
}

