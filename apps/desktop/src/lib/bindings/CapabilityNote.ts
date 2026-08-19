
import type { NoteSeverity } from "./NoteSeverity";

export type CapabilityNote = { severity: NoteSeverity, title: string, detail: string, remedy: string | null, };
