
import type { ActionItem } from "./ActionItem";
import type { TokenUsage } from "./TokenUsage";

export type Summary = { provider: string, model: string, templateId: string, bodyMd: string, actionItems: Array<ActionItem>, usage: TokenUsage, elapsedMs: number, redacted: number, };
