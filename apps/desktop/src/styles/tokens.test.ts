import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";


const tokens = readFileSync("src/styles/tokens.css", "utf8");


function keyframeBlocks(css: string): Map<string, string> {
  const blocks = new Map<string, string>();

  for (const match of css.matchAll(/@keyframes\s+([\w-]+)\s*\{/g)) {
    const start = match.index + match[0].length;


    let depth = 1;
    let index = start;
    while (index < css.length && depth > 0) {
      if (css[index] === "{") depth += 1;
      if (css[index] === "}") depth -= 1;
      index += 1;
    }

    blocks.set(match[1]!, css.slice(start, index - 1));
  }

  return blocks;
}

describe("keyframes", () => {
  it("finds the keyframes it is meant to be checking", () => {
    const names = [...keyframeBlocks(tokens).keys()];
    expect(names).toContain("dialog-in");
    expect(names).toContain("scale-in");
    expect(names.length).toBeGreaterThanOrEqual(6);
  });


  it("animate the individual transform properties, never the shorthand", () => {
    const offenders = [...keyframeBlocks(tokens)]
      .filter(([, body]) => /(^|[\s;{])transform\s*:/.test(body))
      .map(([name]) => name);

    expect(offenders).toEqual([]);
  });
});
