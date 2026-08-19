import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";


const ROOT = join(__dirname, "..", "..", "..", "..");
const SOURCE = join(__dirname, "..");


function sources(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return entry.name === "dev" ? [] : sources(path);
    return /\.tsx?$/.test(entry.name) && !entry.name.endsWith(".test.ts") ? [path] : [];
  });
}

function pluginsUsedByTheInterface(): Set<string> {
  const found = new Set<string>();
  for (const file of sources(SOURCE)) {
    const text = readFileSync(file, "utf8");
    for (const match of text.matchAll(/@tauri-apps\/plugin-([a-z-]+)/g)) {
      found.add(match[1]!);
    }
  }
  return found;
}

describe("the commands the interface calls", () => {
  const ipc = readFileSync(join(SOURCE, "lib", "ipc.ts"), "utf8");
  const lib = readFileSync(join(ROOT, "src-tauri", "src", "lib.rs"), "utf8");

  const called = [...ipc.matchAll(/call<[^>]*>\(\s*"([a-z_0-9]+)"/g)].map((m) => m[1]!);
  const handler = /generate_handler!\[([\s\S]*?)\]/.exec(lib)?.[1] ?? "";
  const registered = new Set(
    [...handler.matchAll(/commands::([a-z_0-9]+)/g)].map((m) => m[1]!),
  );

  it("finds them", () => {
    expect(called.length).toBeGreaterThan(30);
    expect(registered.size).toBeGreaterThan(30);
  });

  it.each([...new Set(called)])("%s is registered in lib.rs", (command) => {
    expect(
      registered.has(command),
      `ipc.ts calls "${command}", which is not in generate_handler![…]. Tauri answers with ` +
        `"Command ${command} not found" at runtime — and only in a real build.`,
    ).toBe(true);
  });
});

describe("the plugins the interface uses", () => {
  const used = pluginsUsedByTheInterface();
  const rust = readFileSync(join(ROOT, "src-tauri", "src", "lib.rs"), "utf8");
  const capability = JSON.parse(
    readFileSync(join(ROOT, "src-tauri", "capabilities", "default.json"), "utf8"),
  ) as { permissions: string[] };

  it("finds the plugins actually in use", () => {


    expect(used.size).toBeGreaterThan(0);
    expect(used).toContain("dialog");
  });

  it.each([...used])("registers %s in lib.rs", (plugin) => {
    const crate = `tauri_plugin_${plugin.replace(/-/g, "_")}`;
    expect(
      rust.includes(`${crate}::init`) || rust.includes(`${crate}::Builder`),
      `The interface imports @tauri-apps/plugin-${plugin}, but ${crate} is never registered in ` +
        `lib.rs. Every call to it fails at runtime with "Plugin not found" — and only in a real ` +
        `build, because the harness mock answers anyway.`,
    ).toBe(true);
  });

  it.each([...used])("grants %s a permission", (plugin) => {
    expect(
      capability.permissions.some((permission) => permission.startsWith(`${plugin}:`)),
      `The interface imports @tauri-apps/plugin-${plugin}, but capabilities/default.json grants ` +
        `it nothing. Calls are rejected by the ACL at runtime.`,
    ).toBe(true);
  });
});
