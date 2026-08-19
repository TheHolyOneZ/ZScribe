import type { Segment } from "./ipc";


export function lineAt(segments: Segment[], ms: number): number {


  let low = 0;
  let high = segments.length - 1;
  let found = -1;

  while (low <= high) {
    const middle = (low + high) >> 1;

    if (segments[middle]!.startMs <= ms) {
      found = middle;
      low = middle + 1;
    } else {
      high = middle - 1;
    }
  }

  return found;
}
