import type { ReactNode } from "react";
import { cn } from "@/lib/cn";


export function SettingGroup({
  title,
  description,
  children,
  className,
}: {
  title?: string;
  description?: string;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section className={cn("mb-8 last:mb-0", className)}>
      {title ? (
        <header className="mb-3">


          <h2 className="text-xs font-medium tracking-wide text-muted uppercase">{title}</h2>
          {description ? <p className="mt-1 text-xs text-faint">{description}</p> : null}
        </header>
      ) : null}
      <div className="divide-y divide-line-subtle overflow-hidden rounded-lg border border-line-subtle bg-surface">
        {children}
      </div>
    </section>
  );
}

export function SettingRow({
  label,
  description,
  htmlFor,
  control,

  stacked = false,
  className,
}: {
  label: string;
  description?: ReactNode;
  htmlFor?: string;
  control: ReactNode;
  stacked?: boolean;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "px-4 py-3",
        stacked ? "space-y-2.5" : "flex items-center justify-between gap-6",
        className,
      )}
    >
      <div className="min-w-0">
        <label
          htmlFor={htmlFor}
          className="block text-sm leading-tight font-medium text-fg"
        >
          {label}
        </label>


        {description ? (
          <div className="mt-1 text-xs leading-relaxed text-muted">{description}</div>
        ) : null}
      </div>
      <div className={cn(stacked ? "w-full" : "shrink-0")}>{control}</div>
    </div>
  );
}


export function Panel({
  title,
  description,
  actions,
  children,
}: {
  title: string;
  description?: string | undefined;
  actions?: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="scroll-area h-full">
      <div className="mx-auto max-w-2xl px-8 py-7">
        <header className="mb-7 flex items-start justify-between gap-6">
          <div className="min-w-0">
            <h1 className="text-lg font-medium text-fg">{title}</h1>
            {description ? (
              <p className="mt-1 text-sm leading-relaxed text-muted">{description}</p>
            ) : null}
          </div>
          {actions ? <div className="flex shrink-0 items-center gap-2">{actions}</div> : null}
        </header>
        {children}
      </div>
    </div>
  );
}
