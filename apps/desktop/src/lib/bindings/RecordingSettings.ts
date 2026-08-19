
import type { AudioSource } from "./AudioSource";
import type { SourceProfile } from "./SourceProfile";

export type RecordingSettings = { source: AudioSource, inputDevice: string | null, sources: Array<SourceProfile>, announceTone: boolean, consentNote: boolean, keepAudio: boolean, rewindSeconds: number, };
