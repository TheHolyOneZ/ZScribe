import type { ComponentType, ReactNode } from "react";
import { cn } from "@/lib/cn";

export interface NavItem<T extends string = string> {
  id: T;
  label: string;

  icon: ComponentType<{ className?: string }>;
}

export function SidebarNav<T extends string>({
  items,
  active,
  onSelect,
  header,
  footer,
}: {
  items: NavItem<T>[];
  active: T;
  onSelect: (id: T) => void;
  header?: ReactNode;
  footer?: ReactNode;
}) {
  return (
    <nav className="flex h-full w-52 shrink-0 flex-col border-r border-line-subtle bg-canvas">
      {header ? <div className="px-3 pt-3 pb-2">{header}</div> : null}

      <div className="scroll-area flex-1 px-2 py-1">
        {items.map((item) => {
          const Icon = item.icon;
          const isActive = item.id === active;
          return (
            <button
              key={item.id}
              onClick={() => onSelect(item.id)}
              aria-current={isActive ? "page" : undefined}
              className={cn(
                "mb-0.5 flex h-control-md w-full items-center gap-2.5 rounded-md px-2.5 text-sm",
                "transition-colors duration-fast ease-out",
                isActive
                  ? "bg-hover font-medium text-fg"
                  : "font-normal text-muted hover:bg-hover/60 hover:text-fg",
              )}
            >
              <Icon className={cn("size-4 shrink-0", isActive ? "text-fg" : "text-faint")} />
              <span className="truncate">{item.label}</span>
            </button>
          );
        })}
      </div>

      {footer ? <div className="border-t border-line-subtle px-3 py-2.5">{footer}</div> : null}
    </nav>
  );
}
