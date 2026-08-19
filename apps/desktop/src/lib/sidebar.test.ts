import { describe, expect, it } from "vitest";
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
} from "./sidebar";

function layout(): SidebarLayout {
  return {
    categories: [
      { id: "a", name: "Active", collapsed: false, items: ["hotkeys", "personas"] },
      { id: "b", name: "Rest", collapsed: true, items: ["usage", "about"] },
    ],
  };
}

const all = (l: SidebarLayout) => l.categories.flatMap((c) => c.items);

describe("moveItem", () => {
  it("reorders within a category", () => {
    const next = moveItem(layout(), "personas", "a", 0);
    expect(next.categories[0]!.items).toEqual(["personas", "hotkeys"]);
  });

  it("moves between categories", () => {
    const next = moveItem(layout(), "hotkeys", "b", 0);
    expect(next.categories[0]!.items).toEqual(["personas"]);
    expect(next.categories[1]!.items).toEqual(["hotkeys", "usage", "about"]);
  });


  it("does not lose or duplicate an entry moved onto itself", () => {
    const next = moveItem(layout(), "hotkeys", "a", 0);
    expect(all(next).filter((i) => i === "hotkeys")).toHaveLength(1);
    expect(all(next)).toHaveLength(4);
  });

  it("clamps an out-of-range index instead of creating holes", () => {
    const next = moveItem(layout(), "hotkeys", "b", 999);
    expect(next.categories[1]!.items).toEqual(["usage", "about", "hotkeys"]);
  });

  it("ignores an unknown category", () => {
    const before = layout();
    expect(moveItem(before, "hotkeys", "nope", 0)).toEqual(before);
  });

  it("does not mutate the input", () => {
    const before = layout();
    moveItem(before, "hotkeys", "b", 0);
    expect(before.categories[0]!.items).toEqual(["hotkeys", "personas"]);
  });
});

describe("moveItemToCategory", () => {
  it("appends to the end", () => {
    const next = moveItemToCategory(layout(), "hotkeys", "b");
    expect(next.categories[1]!.items).toEqual(["usage", "about", "hotkeys"]);
  });
});

describe("moveCategory", () => {
  it("reorders groups", () => {
    const next = moveCategory(layout(), "b", 0);
    expect(next.categories.map((c) => c.id)).toEqual(["b", "a"]);
  });

  it("ignores an unknown category", () => {
    const before = layout();
    expect(moveCategory(before, "nope", 0)).toEqual(before);
  });
});

describe("deleteCategory", () => {

  it("rehomes entries into the first surviving category", () => {
    const next = deleteCategory(layout(), "b");
    expect(next.categories).toHaveLength(1);
    expect(next.categories[0]!.items).toEqual(["hotkeys", "personas", "usage", "about"]);
  });

  it("refuses to delete the last category", () => {
    const single: SidebarLayout = {
      categories: [{ id: "a", name: "All", collapsed: false, items: ["hotkeys"] }],
    };
    expect(deleteCategory(single, "a")).toEqual(single);
  });
});

describe("addCategory / renameCategory / setCollapsed", () => {
  it("adds an empty category", () => {
    const next = addCategory(layout(), "New");
    expect(next.categories).toHaveLength(3);
    expect(next.categories[2]!.items).toEqual([]);
  });

  it("falls back to a placeholder name rather than an unlabelled group", () => {
    expect(addCategory(layout(), "   ").categories[2]!.name).toBe("Untitled");
    expect(renameCategory(layout(), "a", "  ").categories[0]!.name).toBe("Untitled");
  });

  it("toggles collapse", () => {
    expect(setCollapsed(layout(), "a", true).categories[0]!.collapsed).toBe(true);
  });
});

describe("moveItemBy (keyboard reordering)", () => {
  it("moves up within a group", () => {
    const next = moveItemBy(layout(), "personas", -1);
    expect(next.categories[0]!.items).toEqual(["personas", "hotkeys"]);
  });

  it("moves down within a group", () => {
    const next = moveItemBy(layout(), "hotkeys", 1);
    expect(next.categories[0]!.items).toEqual(["personas", "hotkeys"]);
  });


  it("crosses into the next group at its start", () => {
    const next = moveItemBy(layout(), "personas", 1);
    expect(next.categories[0]!.items).toEqual(["hotkeys"]);
    expect(next.categories[1]!.items).toEqual(["personas", "usage", "about"]);
  });


  it("crosses into the previous group at its end", () => {
    const next = moveItemBy(layout(), "usage", -1);
    expect(next.categories[0]!.items).toEqual(["hotkeys", "personas", "usage"]);
    expect(next.categories[1]!.items).toEqual(["about"]);
  });

  it("does nothing at the very top or very bottom", () => {
    const before = layout();
    expect(moveItemBy(before, "hotkeys", -1)).toEqual(before);
    expect(moveItemBy(before, "about", 1)).toEqual(before);
  });

  it("never loses or duplicates an entry", () => {
    let l = layout();
    for (const step of [1, 1, 1, -1, -1, 1] as const) {
      l = moveItemBy(l, "hotkeys", step);
      expect(all(l).sort()).toEqual(["about", "hotkeys", "personas", "usage"]);
    }
  });

  it("ignores an unknown entry", () => {
    const before = layout();
    expect(moveItemBy(before, "missing", 1)).toEqual(before);
  });
});

describe("categoryOf", () => {
  it("finds the owning category", () => {
    expect(categoryOf(layout(), "about")?.id).toBe("b");
    expect(categoryOf(layout(), "missing")).toBeUndefined();
  });
});
