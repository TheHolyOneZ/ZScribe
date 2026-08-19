import { useState, type ComponentType, type DragEvent, type ReactNode } from "react";
import * as ContextMenu from "@radix-ui/react-context-menu";
import { ChevronRight, FolderPlus, GripVertical, Pencil, RotateCcw, Trash2 } from "lucide-react";
import { cn } from "@/lib/cn";
import {
  addCategory,
  categoryOf,
  deleteCategory,
  moveCategory,
  moveItem,
  moveItemBy,
  moveItemToCategory,
  renameCategory,
  setCollapsed,
  type SidebarLayout,
} from "@/lib/sidebar";
import { Button } from "./Button";
import { MenuContent, MenuItem, MenuLabel, MenuSeparator } from "./ContextMenu";
import { Dialog } from "./Dialog";
import { Input } from "./Input";

export interface SidebarEntry<T extends string = string> {
  id: T;
  label: string;
  icon: ComponentType<{ className?: string }>;
}


type Drag = { kind: "entry" | "category"; id: string } | null;

export function Sidebar<T extends string>({
  entries,
  layout,
  active,
  onSelect,
  onLayoutChange,
  onResetLayout,
  footer,
}: {
  entries: SidebarEntry<T>[];
  layout: SidebarLayout;
  active: T;
  onSelect: (id: T) => void;
  onLayoutChange: (layout: SidebarLayout) => void;
  onResetLayout: () => void;
  footer?: ReactNode;
}) {
  const [drag, setDrag] = useState<Drag>(null);
  const [dropTarget, setDropTarget] = useState<string | null>(null);
  const [renaming, setRenaming] = useState<{ id: string; name: string } | null>(null);

  const byId = new Map(entries.map((entry) => [entry.id as string, entry]));

  const startDrag = (event: DragEvent, payload: NonNullable<Drag>) => {
    event.dataTransfer.effectAllowed = "move";

    event.dataTransfer.setData("text/plain", payload.id);
    setDrag(payload);
  };

  const endDrag = () => {
    setDrag(null);
    setDropTarget(null);
  };

  const dropOnEntry = (event: DragEvent, categoryId: string, index: number) => {
    event.preventDefault();
    event.stopPropagation();
    if (drag?.kind === "entry") onLayoutChange(moveItem(layout, drag.id, categoryId, index));
    endDrag();
  };

  const dropOnCategory = (event: DragEvent, categoryId: string) => {
    event.preventDefault();
    if (drag?.kind === "entry") {
      onLayoutChange(moveItemToCategory(layout, drag.id, categoryId));
    } else if (drag?.kind === "category" && drag.id !== categoryId) {
      const to = layout.categories.findIndex((c) => c.id === categoryId);
      onLayoutChange(moveCategory(layout, drag.id, to));
    }
    endDrag();
  };

  return (
    <ContextMenu.Root>
      <ContextMenu.Trigger asChild>
        <nav className="flex h-full w-52 shrink-0 flex-col border-r border-line-subtle">
          <div className="scroll-area flex-1 px-2 py-2" onDragEnd={endDrag}>
            {layout.categories.map((category) => (
              <section key={category.id} className="mb-1">
                <CategoryHeader
                  name={category.name}
                  count={category.items.length}
                  collapsed={category.collapsed}
                  dragging={drag?.kind === "category" && drag.id === category.id}
                  isDropTarget={dropTarget === category.id}
                  onToggle={() =>
                    onLayoutChange(setCollapsed(layout, category.id, !category.collapsed))
                  }
                  onDragStart={(event) => startDrag(event, { kind: "category", id: category.id })}
                  onDragOver={(event) => {
                    event.preventDefault();
                    setDropTarget(category.id);
                  }}
                  onDrop={(event) => dropOnCategory(event, category.id)}
                  onRename={() => setRenaming({ id: category.id, name: category.name })}
                  onDelete={() => onLayoutChange(deleteCategory(layout, category.id))}
                  onNewCategory={() => onLayoutChange(addCategory(layout, "New group"))}
                  canDelete={layout.categories.length > 1}
                />

                {!category.collapsed &&
                  category.items.map((itemId, index) => {
                    const entry = byId.get(itemId);
                    if (!entry) return null;
                    const Icon = entry.icon;
                    const isActive = entry.id === active;

                    return (
                      <ContextMenu.Root key={itemId}>
                        <ContextMenu.Trigger asChild>
                          <button
                            draggable
                            onDragStart={(event) => startDrag(event, { kind: "entry", id: itemId })}
                            onDragOver={(event) => {
                              event.preventDefault();
                              event.stopPropagation();
                              setDropTarget(itemId);
                            }}
                            onDrop={(event) => dropOnEntry(event, category.id, index)}
                            onClick={() => onSelect(entry.id)}


                            onKeyDown={(event) => {
                              if (!event.altKey) return;
                              const delta =
                                event.key === "ArrowUp"
                                  ? -1
                                  : event.key === "ArrowDown"
                                    ? 1
                                    : null;
                              if (delta === null) return;

                              event.preventDefault();
                              onLayoutChange(moveItemBy(layout, itemId, delta));


                              requestAnimationFrame(() => {
                                document
                                  .querySelector<HTMLElement>(`[data-entry="${itemId}"]`)
                                  ?.focus();
                              });
                            }}
                            data-entry={itemId}
                            aria-current={isActive ? "page" : undefined}
                            aria-keyshortcuts="Alt+ArrowUp Alt+ArrowDown"
                            className={cn(
                              "group mb-0.5 flex h-control-md w-full items-center gap-2.5 rounded-md px-2.5 text-sm",
                              "transition-colors duration-fast ease-out",
                              isActive
                                ? "bg-hover font-medium text-fg"
                                : "font-normal text-muted hover:bg-hover/60 hover:text-fg",
                              drag?.kind === "entry" && drag.id === itemId && "opacity-40",
                              dropTarget === itemId &&
                                drag?.id !== itemId &&
                                "ring-1 ring-accent ring-inset",
                            )}
                          >
                            <GripVertical
                              className={cn(
                                "size-3 shrink-0 text-faint opacity-0 transition-opacity duration-fast",
                                "group-hover:opacity-100",
                              )}
                            />
                            <Icon
                              className={cn("size-4 shrink-0", isActive ? "text-fg" : "text-faint")}
                            />
                            <span className="truncate">{entry.label}</span>
                          </button>
                        </ContextMenu.Trigger>

                        <MenuContent>
                          <MenuLabel>Move “{entry.label}” to</MenuLabel>
                          {layout.categories.map((target) => (
                            <MenuItem
                              key={target.id}
                              disabled={categoryOf(layout, itemId)?.id === target.id}
                              onSelect={() =>
                                onLayoutChange(moveItemToCategory(layout, itemId, target.id))
                              }
                            >
                              {target.name}
                            </MenuItem>
                          ))}
                          <MenuSeparator />
                          <MenuItem
                            icon={<FolderPlus />}
                            onSelect={() => onLayoutChange(addCategory(layout, "New group"))}
                          >
                            New group
                          </MenuItem>
                        </MenuContent>
                      </ContextMenu.Root>
                    );
                  })}


                {!category.collapsed && category.items.length === 0 ? (
                  <div
                    onDragOver={(event) => {
                      event.preventDefault();
                      setDropTarget(category.id);
                    }}
                    onDrop={(event) => dropOnCategory(event, category.id)}
                    className={cn(
                      "mb-0.5 rounded-md border border-dashed px-2.5 py-2 text-2xs",
                      dropTarget === category.id
                        ? "border-accent text-accent"
                        : "border-line text-faint",
                    )}
                  >
                    Drop a panel here
                  </div>
                ) : null}
              </section>
            ))}
          </div>


          <button
            type="button"
            onClick={() => onLayoutChange(addCategory(layout, "New group"))}
            className="mx-2 mb-2 flex items-center gap-1.5 rounded-md px-2 py-1 text-2xs text-faint transition-colors duration-fast ease-out hover:bg-hover/60 hover:text-muted [&_svg]:size-3"
          >
            <FolderPlus />
            New group
          </button>

          {footer ? <div className="border-t border-line-subtle px-3 py-2.5">{footer}</div> : null}
        </nav>
      </ContextMenu.Trigger>

      <MenuContent>
        <MenuItem icon={<FolderPlus />} onSelect={() => onLayoutChange(addCategory(layout, "New group"))}>
          New group
        </MenuItem>
        <MenuSeparator />
        <MenuItem icon={<RotateCcw />} onSelect={onResetLayout}>
          Reset sidebar layout
        </MenuItem>
      </MenuContent>

      <Dialog
        open={renaming !== null}
        onOpenChange={(open) => !open && setRenaming(null)}
        title="Rename group"
        width="sm"
        footer={
          <>
            <Button variant="ghost" onClick={() => setRenaming(null)}>
              Cancel
            </Button>
            <Button
              variant="primary"
              onClick={() => {
                if (renaming) onLayoutChange(renameCategory(layout, renaming.id, renaming.name));
                setRenaming(null);
              }}
            >
              Rename
            </Button>
          </>
        }
      >
        <Input
          value={renaming?.name ?? ""}
          onChange={(event) =>
            setRenaming((current) => (current ? { ...current, name: event.target.value } : current))
          }
          autoFocus
          aria-label="Group name"
        />
      </Dialog>
    </ContextMenu.Root>
  );
}

function CategoryHeader({
  name,
  count,
  collapsed,
  dragging,
  isDropTarget,
  onToggle,
  onDragStart,
  onDragOver,
  onDrop,
  onRename,
  onDelete,
  onNewCategory,
  canDelete,
}: {
  name: string;
  count: number;
  collapsed: boolean;
  dragging: boolean;
  isDropTarget: boolean;
  onToggle: () => void;
  onDragStart: (event: DragEvent) => void;
  onDragOver: (event: DragEvent) => void;
  onDrop: (event: DragEvent) => void;
  onRename: () => void;
  onDelete: () => void;
  onNewCategory: () => void;
  canDelete: boolean;
}) {
  return (
    <ContextMenu.Root>
      <ContextMenu.Trigger asChild>
        <button
          draggable
          onDragStart={onDragStart}
          onDragOver={onDragOver}
          onDrop={onDrop}
          onClick={onToggle}
          className={cn(
            "mb-1 flex w-full items-center gap-1 rounded-md px-1.5 py-1",
            "text-2xs font-medium tracking-wide text-faint uppercase",
            "transition-colors duration-fast ease-out hover:text-muted",
            dragging && "opacity-40",
            isDropTarget && "ring-1 ring-accent ring-inset",
          )}
        >
          <ChevronRight
            className={cn(
              "size-3 shrink-0 transition-transform duration-fast ease-out",
              !collapsed && "rotate-90",
            )}
          />
          <span className="truncate">{name}</span>
          {collapsed && count > 0 ? (
            <span data-numeric className="ml-auto text-faint">
              {count}
            </span>
          ) : null}
        </button>
      </ContextMenu.Trigger>

      <MenuContent>
        <MenuItem icon={<Pencil />} onSelect={onRename}>
          Rename group
        </MenuItem>
        <MenuItem icon={<FolderPlus />} onSelect={onNewCategory}>
          New group
        </MenuItem>
        <MenuSeparator />
        <MenuItem icon={<Trash2 />} tone="danger" disabled={!canDelete} onSelect={onDelete}>
          Delete group
        </MenuItem>
      </MenuContent>
    </ContextMenu.Root>
  );
}

