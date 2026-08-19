import { cn } from "@/lib/cn";


export const CONTENT_CLASS = cn(
  "z-50 min-w-48 overflow-hidden rounded-lg p-1",
  "border border-line bg-surface shadow-popover",
  "data-[state=open]:animate-scale-in",
);

export const LABEL_CLASS =
  "truncate px-2 py-1.5 text-2xs font-medium tracking-wide text-faint uppercase";

export const SEPARATOR_CLASS = "my-1 h-px bg-line-subtle";

export const ITEM_CLASS = (tone: "default" | "danger" = "default") =>
  cn(
    "flex cursor-default items-center gap-2 rounded-md px-2 py-1.5 text-sm outline-none",
    "data-[disabled]:pointer-events-none data-[disabled]:opacity-40",
    "[&_svg]:size-3.5 [&_svg]:shrink-0",
    tone === "danger"
      ? "text-danger data-[highlighted]:bg-danger-subtle [&_svg]:text-danger"
      : "text-fg data-[highlighted]:bg-hover [&_svg]:text-faint",
  );
