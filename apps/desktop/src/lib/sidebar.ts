

import type { SidebarCategory } from "./bindings/SidebarCategory";
import type { SidebarLayout } from "./bindings/SidebarLayout";

export type { SidebarCategory, SidebarLayout };


function clone(layout: SidebarLayout): SidebarLayout {
  return { categories: layout.categories.map((c) => ({ ...c, items: [...c.items] })) };
}


function detach(layout: SidebarLayout, item: string): SidebarLayout {
  for (const category of layout.categories) {
    category.items = category.items.filter((existing) => existing !== item);
  }
  return layout;
}


export function moveItem(
  layout: SidebarLayout,
  item: string,
  categoryId: string,
  index: number,
): SidebarLayout {
  const next = detach(clone(layout), item);
  const target = next.categories.find((c) => c.id === categoryId);
  if (!target) return layout;

  target.items.splice(Math.max(0, Math.min(index, target.items.length)), 0, item);
  return next;
}


export function moveItemToCategory(
  layout: SidebarLayout,
  item: string,
  categoryId: string,
): SidebarLayout {
  const target = layout.categories.find((c) => c.id === categoryId);
  return moveItem(layout, item, categoryId, target ? target.items.length : 0);
}


export function moveCategory(
  layout: SidebarLayout,
  categoryId: string,
  toIndex: number,
): SidebarLayout {
  const next = clone(layout);
  const from = next.categories.findIndex((c) => c.id === categoryId);
  if (from === -1) return layout;

  const [moved] = next.categories.splice(from, 1);
  if (!moved) return layout;
  next.categories.splice(Math.max(0, Math.min(toIndex, next.categories.length)), 0, moved);
  return next;
}

export function setCollapsed(
  layout: SidebarLayout,
  categoryId: string,
  collapsed: boolean,
): SidebarLayout {
  const next = clone(layout);
  const target = next.categories.find((c) => c.id === categoryId);
  if (target) target.collapsed = collapsed;
  return next;
}

export function renameCategory(
  layout: SidebarLayout,
  categoryId: string,
  name: string,
): SidebarLayout {
  const next = clone(layout);
  const target = next.categories.find((c) => c.id === categoryId);
  if (target) target.name = name.trim() || "Untitled";
  return next;
}

export function addCategory(layout: SidebarLayout, name: string): SidebarLayout {
  const next = clone(layout);
  next.categories.push({
    id: crypto.randomUUID(),
    name: name.trim() || "Untitled",
    collapsed: false,
    items: [],
  });
  return next;
}


export function deleteCategory(layout: SidebarLayout, categoryId: string): SidebarLayout {
  if (layout.categories.length <= 1) return layout;

  const next = clone(layout);
  const index = next.categories.findIndex((c) => c.id === categoryId);
  if (index === -1) return layout;

  const [removed] = next.categories.splice(index, 1);
  const survivor = next.categories[0];
  if (removed && survivor) survivor.items.push(...removed.items);
  return next;
}


export function moveItemBy(
  layout: SidebarLayout,
  item: string,
  delta: -1 | 1,
): SidebarLayout {
  const categoryIndex = layout.categories.findIndex((c) => c.items.includes(item));
  if (categoryIndex === -1) return layout;

  const category = layout.categories[categoryIndex]!;
  const index = category.items.indexOf(item);

  if (delta === -1) {
    if (index > 0) return moveItem(layout, item, category.id, index - 1);

    const previous = layout.categories[categoryIndex - 1];

    return previous ? moveItem(layout, item, previous.id, previous.items.length) : layout;
  }

  if (index < category.items.length - 1) {
    return moveItem(layout, item, category.id, index + 1);
  }

  const next = layout.categories[categoryIndex + 1];

  return next ? moveItem(layout, item, next.id, 0) : layout;
}


export function categoryOf(layout: SidebarLayout, item: string): SidebarCategory | undefined {
  return layout.categories.find((c) => c.items.includes(item));
}
