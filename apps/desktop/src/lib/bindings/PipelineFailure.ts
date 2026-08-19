
import type { ProviderErrorInfo } from "./ProviderErrorInfo";

export type PipelineFailure = { recordingId: string, stage: string, error: ProviderErrorInfo, };
