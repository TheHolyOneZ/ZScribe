

export type Recording = { id: string, startedAt: number, durationMs: number, source: string, templateId: string, title: string, audioPath: string | null, hasTranscript: boolean, hasSummary: boolean, tags: Array<string>, };
