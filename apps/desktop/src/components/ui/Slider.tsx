import * as RSlider from "@radix-ui/react-slider";
import { cn } from "@/lib/cn";

export interface SliderProps {
  value: number;
  onValueChange: (value: number) => void;
  onValueCommit?: (value: number) => void;
  min?: number;
  max?: number;
  step?: number;
  disabled?: boolean;
  className?: string;
  "aria-label"?: string;
}

export function Slider({
  value,
  onValueChange,
  onValueCommit,
  min = 0,
  max = 100,
  step = 1,
  disabled,
  className,
  ...aria
}: SliderProps) {
  return (
    <RSlider.Root
      value={[value]}
      onValueChange={([v]) => onValueChange(v ?? min)}
      {...(onValueCommit ? { onValueCommit: ([v]: number[]) => onValueCommit(v ?? min) } : {})}
      min={min}
      max={max}
      step={step}
      disabled={disabled ?? false}
      aria-label={aria["aria-label"]}
      className={cn(
        "relative flex h-4 w-full touch-none items-center select-none",
        "data-[disabled]:pointer-events-none data-[disabled]:opacity-40",
        className,
      )}
    >
      <RSlider.Track className="relative h-1 w-full grow overflow-hidden rounded-full bg-line">
        <RSlider.Range className="absolute h-full bg-accent" />
      </RSlider.Track>
      <RSlider.Thumb
        className={cn(
          "block size-3.5 rounded-full bg-white",
          "shadow-[0_1px_3px_oklch(0_0_0/0.3)]",
          "transition-transform duration-fast ease-out hover:scale-110",
        )}
      />
    </RSlider.Root>
  );
}
