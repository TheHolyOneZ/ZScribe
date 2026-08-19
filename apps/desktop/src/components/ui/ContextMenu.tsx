import type { ReactNode } from "react";
import * as Radix from "@radix-ui/react-context-menu";

import { CONTENT_CLASS, ITEM_CLASS, LABEL_CLASS, SEPARATOR_CLASS } from "./menuStyles";


export function ContextMenu({
  children,
  content,
  disabled,
}: {

  children: ReactNode;
  content: ReactNode;
  disabled?: boolean;
}) {
  if (disabled) return <>{children}</>;

  return (
    <Radix.Root>
      <Radix.Trigger asChild>{children}</Radix.Trigger>
      <MenuContent>{content}</MenuContent>
    </Radix.Root>
  );
}

export function MenuContent({ children }: { children: ReactNode }) {
  return (
    <Radix.Portal>
      <Radix.Content


        onCloseAutoFocus={(event) => event.preventDefault()}
        className={CONTENT_CLASS}
      >
        {children}
      </Radix.Content>
    </Radix.Portal>
  );
}

export function MenuLabel({ children }: { children: ReactNode }) {
  return (
    <Radix.Label className={LABEL_CLASS}>
      {children}
    </Radix.Label>
  );
}

export function MenuSeparator() {
  return <Radix.Separator className={SEPARATOR_CLASS} />;
}

export function MenuItem({
  children,
  icon,
  shortcut,
  disabled,
  tone = "default",
  onSelect,
}: {
  children: ReactNode;
  icon?: ReactNode;

  shortcut?: string;
  disabled?: boolean;

  tone?: "default" | "danger";
  onSelect: () => void;
}) {
  return (
    <Radix.Item
      disabled={disabled ?? false}
      onSelect={onSelect}
      className={ITEM_CLASS(tone)}
    >
      {icon}
      <span className="min-w-0 flex-1 truncate">{children}</span>
      {shortcut ? (
        <span data-numeric className="shrink-0 text-2xs text-faint">
          {shortcut}
        </span>
      ) : null}
    </Radix.Item>
  );
}
