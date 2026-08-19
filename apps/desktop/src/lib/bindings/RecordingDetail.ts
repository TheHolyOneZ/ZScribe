
import type { Recording } from "./Recording";
import type { Summary } from "./Summary";
import type { Transcript } from "./Transcript";

export type RecordingDetail = { recording: Recording, transcript: Transcript | null, summary: Summary | null, };
