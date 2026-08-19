import * as RDialog from "@radix-ui/react-dialog";
import { X } from "lucide-react";
import type { ReactNode } from "react";
import { cn } from "@/lib/cn";
import { Button } from "./Button";

export interface DialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;

  description?: string;
  children: ReactNode;

  footer?: ReactNode;
  width?: "sm" | "md" | "lg";
}

const WIDTHS = { sm: "max-w-sm", md: "max-w-md", lg: "max-w-lg" } as const;

export function Dialog({
  open,
  onOpenChange,
  title,
  description,
  children,
  footer,
  width = "md",
}: DialogProps) {
  return (
    <RDialog.Root open={open} onOpenChange={onOpenChange}>
      <RDialog.Portal>


        <RDialog.Overlay
          className={cn(
            "fixed inset-0 z-50 grid place-items-center overflow-y-auto p-6",
            "bg-black/45 data-[state=open]:animate-fade-in",
          )}
        >
          <RDialog.Content
            className={cn(
              "relative w-full",
              "rounded-xl border border-line bg-surface shadow-overlay",
              "data-[state=open]:animate-dialog-in",
              WIDTHS[width],
            )}
          >
            <header className="px-5 pt-4 pb-3">
              <div className="flex items-start justify-between gap-4">
                <RDialog.Title className="text-base font-medium text-fg">
                  {title}
                </RDialog.Title>
                <RDialog.Close asChild>
                  <Button variant="ghost" size="sm" icon aria-label="Close">
                    <X />
                  </Button>
                </RDialog.Close>
              </div>
              {description ? (
                <RDialog.Description className="mt-1 text-xs text-muted">
                  {description}
                </RDialog.Description>
              ) : null}
            </header>

            <div className="scroll-area max-h-[60vh] px-5 pb-4">{children}</div>

            {footer ? (
              <footer className="flex items-center justify-end gap-2 border-t border-line-subtle px-5 py-3">
                {footer}
              </footer>
            ) : null}
          </RDialog.Content>
        </RDialog.Overlay>
      </RDialog.Portal>
    </RDialog.Root>
  );
}
