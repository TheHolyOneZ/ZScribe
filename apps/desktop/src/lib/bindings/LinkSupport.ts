
import type { Platform } from "./Platform";

export type LinkSupport = { available: boolean, path: string | null, version: string | null, versionAgeDays: number | null, staleAfterDays: number, installCommand: string, nightlyCommand: string | null, standaloneAsset: string, toolsDir: string, toolsPath: string, platform: Platform, jsRuntime: string | null, jsRuntimeCommand: string, };
