
import type { AppearanceSettings } from "./AppearanceSettings";
import type { ArchiveSettings } from "./ArchiveSettings";
import type { FolderSettings } from "./FolderSettings";
import type { PrivacySettings } from "./PrivacySettings";
import type { ProviderId } from "./ProviderId";
import type { ProviderProfile } from "./ProviderProfile";
import type { RecordingSettings } from "./RecordingSettings";
import type { SidebarLayout } from "./SidebarLayout";
import type { SystemSettings } from "./SystemSettings";
import type { Template } from "./Template";
import type { TranscriptionSettings } from "./TranscriptionSettings";

export type AppSettings = { schemaVersion: number, hotkey: string, templateId: string, customTemplates: Array<Template>, summaryLanguage: string, activeProvider: ProviderId, providers: Array<ProviderProfile>, recording: RecordingSettings, transcription: TranscriptionSettings, archive: ArchiveSettings, privacy: PrivacySettings, folders: FolderSettings, system: SystemSettings, appearance: AppearanceSettings, sidebar: SidebarLayout, consentAcknowledged: boolean, };
