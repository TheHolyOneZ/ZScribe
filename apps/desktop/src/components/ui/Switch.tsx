import * as RSwitch from "@radix-ui/react-switch";
import { cn } from "@/lib/cn";

export interface SwitchProps {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  disabled?: boolean;
  id?: string;
  "aria-label"?: string;
}

export function Switch({ checked, onCheckedChange, disabled, id, ...aria }: SwitchProps) {
  return (
    <RSwitch.Root
      id={id}
      checked={checked}
      onCheckedChange={onCheckedChange}
      disabled={disabled}
      aria-label={aria["aria-label"]}
      className={cn(
        "peer inline-flex h-[1.125rem] w-8 shrink-0 items-center rounded-full",
        "border border-transparent transition-colors duration-fast ease-out",
        "bg-line-strong data-[state=checked]:bg-accent",
        "disabled:pointer-events-none disabled:opacity-40",
      )}
    >
      <RSwitch.Thumb
        className={cn(
          "pointer-events-none block size-3.5 rounded-full bg-white shadow-sm",
          "transition-transform duration-fast ease-out",
          "translate-x-0.5 data-[state=checked]:translate-x-[0.875rem]",
        )}
      />
    </RSwitch.Root>
  );
}
