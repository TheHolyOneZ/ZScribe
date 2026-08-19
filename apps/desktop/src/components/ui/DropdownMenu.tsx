import type { ReactNode } from "react";
import * as Radix from "@radix-ui/react-dropdown-menu";

import { cn } from "@/lib/cn";
import { CONTENT_CLASS, ITEM_CLASS, LABEL_CLASS, SEPARATOR_CLASS } from "./menuStyles";


export function DropdownMenu({
  trigger,
  children,
  align = "end",
}: {

  trigger: ReactNode;
  children: ReactNode;
  align?: "start" | "center" | "end";
}) {
  return (
    <Radix.Root>
      <Radix.Trigger asChild>{trigger}</Radix.Trigger>
      <Radix.Portal>
        <Radix.Content align={align} sideOffset={6} className={CONTENT_CLASS}>
          {children}
        </Radix.Content>
      </Radix.Portal>
    </Radix.Root>
  );
}

export function DropdownLabel({ children }: { children: ReactNode }) {
  return <Radix.Label className={LABEL_CLASS}>{children}</Radix.Label>;
}

export function DropdownSeparator() {
  return <Radix.Separator className={SEPARATOR_CLASS} />;
}

export function DropdownItem({
  children,
  icon,
  description,
  disabled,
  tone = "default",
  onSelect,
}: {
  children: ReactNode;
  icon?: ReactNode;


  description?: string;

  disabled?: boolean;
  tone?: "default" | "danger";
  onSelect: () => void;
}) {
  return (
    <Radix.Item
      disabled={disabled ?? false}
      onSelect={onSelect}
      className={cn(ITEM_CLASS(tone), description && "items-start py-2")}
    >
      {icon}
      <span className="min-w-0 flex-1">
        <span className="block truncate">{children}</span>
        {description ? (
          <span className="mt-0.5 block text-2xs text-faint">{description}</span>
        ) : null}
      </span>
    </Radix.Item>
  );
}
