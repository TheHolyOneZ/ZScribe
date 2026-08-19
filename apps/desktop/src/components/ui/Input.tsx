import { forwardRef, type InputHTMLAttributes, type TextareaHTMLAttributes } from "react";
import { cn } from "@/lib/cn";

const FIELD_BASE = cn(
  "w-full rounded-md bg-inset text-fg text-sm",
  "border border-line placeholder:text-faint",
  "transition-colors duration-fast ease-out",
  "hover:border-line-strong",
  "focus:border-accent focus:outline-none focus:ring-2 focus:ring-accent/25",
  "disabled:opacity-40 disabled:pointer-events-none",
);

export const Input = forwardRef<HTMLInputElement, InputHTMLAttributes<HTMLInputElement>>(
  function Input({ className, ...props }, ref) {
    return <input ref={ref} className={cn(FIELD_BASE, "h-control-md px-2.5", className)} {...props} />;
  },
);

export const Textarea = forwardRef<HTMLTextAreaElement, TextareaHTMLAttributes<HTMLTextAreaElement>>(
  function Textarea({ className, ...props }, ref) {
    return (
      <textarea
        ref={ref}
        className={cn(FIELD_BASE, "resize-none px-2.5 py-2 leading-relaxed", className)}
        {...props}
      />
    );
  },
);
