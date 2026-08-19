
import type { Acceleration } from "./Acceleration";
import type { Gpu } from "./Gpu";

export type Machine = { cpuModel: string, cpuThreads: number, totalRamMb: number, availableRamMb: number, freeDiskMb: number, acceleration: Acceleration, gpus: Array<Gpu>, };
