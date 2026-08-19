import type { ReactNode } from "react";
import { cn } from "@/lib/cn";

export type Tone = "neutral" | "accent" | "success" | "warning" | "danger";

const DOT_TONES: Record<Tone, string> = {
  neutral: "bg-faint",
  accent: "bg-accent",
  success: "bg-success",
  warning: "bg-warning",
  danger: "bg-danger",
};


export function StatusDot({
  tone = "neutral",
  children,
  className,
}: {
  tone?: Tone;
  children: ReactNode;
  className?: string;
}) {
  return (
    <span className={cn("inline-flex items-center gap-2 text-sm text-muted", className)}>
      <span className={cn("size-1.5 shrink-0 rounded-full", DOT_TONES[tone])} aria-hidden />
      {children}
    </span>
  );
}

const CALLOUT_TONES: Record<Tone, string> = {
  neutral: "border-line bg-raised text-muted",
  accent: "border-accent-border bg-accent-subtle text-fg",
  success: "border-success/35 bg-success-subtle text-fg",
  warning: "border-warning/35 bg-warning-subtle text-fg",
  danger: "border-danger/35 bg-danger-subtle text-fg",
};


const ICON_TONES: Record<Tone, string> = {
  neutral: "text-faint",
  accent: "text-accent",
  success: "text-success",
  warning: "text-warning",
  danger: "text-danger",
};


export function Callout({
  tone = "neutral",
  icon,
  title,
  children,
  action,
  className,
}: {
  tone?: Tone;
  icon?: ReactNode;
  title?: string;
  children?: ReactNode;
  action?: ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "flex items-start gap-3 rounded-lg border px-3.5 py-3 text-sm",
        CALLOUT_TONES[tone],
        className,
      )}
    >
      {icon ? (
        <span className={cn("mt-px shrink-0 [&_svg]:size-4", ICON_TONES[tone])}>{icon}</span>
      ) : null}
      <div className="min-w-0 flex-1">
        {title ? <p className="font-medium text-fg">{title}</p> : null}
        {children ? (
          <div className={cn("text-xs leading-relaxed text-muted", title && "mt-1")}>{children}</div>
        ) : null}
      </div>
      {action ? <div className="shrink-0">{action}</div> : null}
    </div>
  );
}


export function EmptyState({
  icon,
  title,
  description,
  action,
}: {
  icon?: ReactNode;
  title: string;
  description?: string;
  action?: ReactNode;
}) {
  return (
    <div className="flex flex-col items-center justify-center px-6 py-12 text-center">
      {icon ? <span className="mb-3 text-faint [&_svg]:size-6">{icon}</span> : null}
      <p className="text-sm font-medium text-fg">{title}</p>
      {description ? (
        <p className="mt-1 max-w-xs text-xs leading-relaxed text-muted">{description}</p>
      ) : null}
      {action ? <div className="mt-4">{action}</div> : null}
    </div>
  );
}


export function Skeleton({ className }: { className?: string }) {
  return <div className={cn("animate-skeleton rounded-md bg-hover", className)} />;
}
