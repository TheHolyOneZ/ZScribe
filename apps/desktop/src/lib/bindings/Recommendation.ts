
import type { CapabilityNote } from "./CapabilityNote";

export type Recommendation = { modelId: string, accelerated: boolean, headline: string, viable: Array<string>, notes: Array<CapabilityNote>, };
