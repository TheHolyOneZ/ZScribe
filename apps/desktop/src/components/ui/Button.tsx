import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from "react";
import { cn } from "@/lib/cn";

type Variant = "primary" | "secondary" | "ghost" | "danger";
type Size = "sm" | "md" | "lg";

const VARIANTS: Record<Variant, string> = {

  primary: "bg-accent text-on-accent hover:bg-accent-hover active:bg-accent-active",
  secondary:
    "bg-raised text-fg border border-line hover:bg-hover hover:border-line-strong active:bg-active",
  ghost: "text-muted hover:bg-hover hover:text-fg active:bg-active",
  danger: "bg-danger text-on-accent hover:bg-danger-hover",
};

const SIZES: Record<Size, string> = {
  sm: "h-control-sm px-2.5 text-xs gap-1.5",
  md: "h-control-md px-3 text-sm gap-2",
  lg: "h-control-lg px-4 text-sm gap-2",
};

const ICON_SIZES: Record<Size, string> = {
  sm: "h-control-sm w-control-sm",
  md: "h-control-md w-control-md",
  lg: "h-control-lg w-control-lg",
};

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  size?: Size;

  icon?: boolean;
  children?: ReactNode;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { variant = "secondary", size = "md", icon = false, className, type = "button", ...props },
  ref,
) {
  return (
    <button
      ref={ref}
      type={type}
      className={cn(
        "inline-flex shrink-0 items-center justify-center rounded-md font-medium whitespace-nowrap",
        "transition-colors duration-fast ease-out",
        "disabled:pointer-events-none disabled:opacity-40",
        "[&_svg]:size-4 [&_svg]:shrink-0",
        icon ? ICON_SIZES[size] : SIZES[size],
        VARIANTS[variant],
        className,
      )}
      {...props}
    />
  );
});
