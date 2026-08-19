
import type { Level } from "./Level";

export type RecordingStatus = { active: boolean, paused: boolean, durationMs: number, level: Level, problem: string | null, rewindMs: number, };
