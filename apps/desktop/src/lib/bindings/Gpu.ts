
import type { GpuKind } from "./GpuKind";

export type Gpu = { name: string, vendor: string, kind: GpuKind, vramMb: number, };
