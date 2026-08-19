import { useState } from "react";
import { Plus, Tag, X } from "lucide-react";

import { Input } from "@/components/ui";
import { cn } from "@/lib/cn";


export function Tags({
  tags,
  onChange,
  noun = "tag",
  placeholder = "client, 1:1, to write up…",
}: {
  tags: string[];
  onChange: (tags: string[]) => Promise<void>;


  noun?: string;
  placeholder?: string;
}) {
  const [adding, setAdding] = useState(false);
  const [draft, setDraft] = useState("");

  const add = async () => {
    const wanted = draft.trim();
    setDraft("");
    setAdding(false);

    if (!wanted) return;
    if (tags.some((tag) => tag.toLowerCase() === wanted.toLowerCase())) return;
    await onChange([...tags, wanted]);
  };

  return (
    <div className="flex flex-wrap items-center gap-1.5">
      {tags.map((tag) => (
        <span
          key={tag}
          className={cn(
            "inline-flex items-center gap-1 rounded-md border border-line-subtle bg-inset",
            "py-0.5 pr-1 pl-2 text-2xs text-muted",
          )}
        >
          {tag}
          <button
            type="button"
            onClick={() => void onChange(tags.filter((existing) => existing !== tag))}
            aria-label={`Remove the ${noun} ${tag}`}
            title={`Remove the ${noun} ${tag}`}
            className={cn(
              "inline-flex size-4 items-center justify-center rounded",
              "text-faint transition-colors duration-fast ease-out",
              "hover:bg-danger-subtle hover:text-danger [&_svg]:size-2.5",
            )}
          >
            <X />
          </button>
        </span>
      ))}

      {adding ? (
        <Input
          autoFocus
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          onBlur={() => void add()}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              void add();
            }
            if (event.key === "Escape") {
              event.preventDefault();
              setDraft("");
              setAdding(false);
            }
          }}
          placeholder={placeholder}
          aria-label={`Add a ${noun}`}
          className="h-6 w-40 text-2xs"
        />
      ) : (
        <button
          type="button"
          onClick={() => setAdding(true)}
          title={`Add a ${noun}`}
          className={cn(
            "inline-flex items-center gap-1 rounded-md border border-dashed border-line-subtle",
            "px-2 py-0.5 text-2xs text-faint",
            "transition-colors duration-fast ease-out hover:border-line hover:text-muted",
            "[&_svg]:size-2.5",
          )}
        >
          {tags.length === 0 ? <Tag /> : <Plus />}
          {tags.length === 0 ? `Add a ${noun}` : "Add"}
        </button>
      )}
    </div>
  );
}


export function TagFilter({
  tags,
  active,
  onPick,
}: {
  tags: [string, number][];
  active: string | null;
  onPick: (tag: string | null) => void;
}) {
  if (tags.length === 0) return null;

  return (
    <div className="scroll-area flex gap-1 overflow-x-auto border-b border-line-subtle px-2.5 py-2">
      {tags.map(([tag, uses]) => {
        const chosen = tag === active;
        return (
          <button
            key={tag}
            type="button"
            onClick={() => onPick(chosen ? null : tag)}
            aria-pressed={chosen}
            className={cn(
              "inline-flex shrink-0 items-center gap-1.5 rounded-md border px-2 py-0.5 text-2xs",
              "transition-colors duration-fast ease-out",
              chosen
                ? "border-accent-border bg-accent-subtle text-fg"
                : "border-line-subtle text-muted hover:bg-hover hover:text-fg",
            )}
          >
            {tag}
            <span data-numeric className="text-faint">
              {uses}
            </span>
          </button>
        );
      })}
    </div>
  );
}
