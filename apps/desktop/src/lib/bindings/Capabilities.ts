
import type { CapabilityNote } from "./CapabilityNote";
import type { DisplayServer } from "./DisplayServer";
import type { HotkeyBackend } from "./HotkeyBackend";

export type Capabilities = { displayServer: DisplayServer, hotkey: HotkeyBackend, notes: Array<CapabilityNote>, };
