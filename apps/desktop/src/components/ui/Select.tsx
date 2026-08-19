import * as RSelect from "@radix-ui/react-select";
import { Check, ChevronDown } from "lucide-react";
import { cn } from "@/lib/cn";

export interface SelectOption {
  value: string;
  label: string;
  hint?: string;
  disabled?: boolean;
}

export interface SelectProps {
  value: string;
  onValueChange: (value: string) => void;
  options: SelectOption[];
  placeholder?: string;
  disabled?: boolean;
  className?: string;
  "aria-label"?: string;
}

export function Select({
  value,
  onValueChange,
  options,
  placeholder = "Select…",
  disabled,
  className,
  ...aria
}: SelectProps) {
  return (
    <RSelect.Root value={value} onValueChange={onValueChange} disabled={disabled ?? false}>
      <RSelect.Trigger
        aria-label={aria["aria-label"]}
        className={cn(
          "inline-flex h-control-md w-full items-center justify-between gap-2 rounded-md px-2.5",
          "bg-inset border border-line text-sm text-fg",
          "transition-colors duration-fast ease-out",
          "hover:border-line-strong data-[state=open]:border-accent",
          "data-[placeholder]:text-faint",
          "disabled:pointer-events-none disabled:opacity-40",
          className,
        )}
      >
        <RSelect.Value placeholder={placeholder} />
        <RSelect.Icon asChild>
          <ChevronDown className="size-3.5 shrink-0 text-faint" />
        </RSelect.Icon>
      </RSelect.Trigger>

      <RSelect.Portal>
        <RSelect.Content
          position="popper"
          sideOffset={4}
          className={cn(
            "z-50 min-w-(--radix-select-trigger-width) overflow-hidden rounded-lg",
            "bg-surface border border-line shadow-popover",
            "data-[state=open]:animate-scale-in",
          )}
        >
          <RSelect.Viewport className="scroll-area max-h-64 p-1">
            {options.map((opt) => (
              <RSelect.Item
                key={opt.value}
                value={opt.value}
                disabled={opt.disabled ?? false}
                className={cn(
                  "relative flex cursor-default items-center gap-2 rounded-md py-1.5 pr-2 pl-7",
                  "text-sm text-fg outline-none select-none",
                  "data-[highlighted]:bg-hover",
                  "data-[disabled]:pointer-events-none data-[disabled]:opacity-40",
                )}
              >
                <RSelect.ItemIndicator className="absolute left-2 inline-flex">
                  <Check className="size-3.5 text-accent" />
                </RSelect.ItemIndicator>
                <RSelect.ItemText>{opt.label}</RSelect.ItemText>
                {opt.hint ? <span className="ml-auto text-xs text-faint">{opt.hint}</span> : null}
              </RSelect.Item>
            ))}
          </RSelect.Viewport>
        </RSelect.Content>
      </RSelect.Portal>
    </RSelect.Root>
  );
}
