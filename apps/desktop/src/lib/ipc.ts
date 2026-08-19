import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { AppSettings } from "./bindings/AppSettings";
import type { Capabilities } from "./bindings/Capabilities";
import type { CapabilityNote } from "./bindings/CapabilityNote";
import type { InputDevice } from "./bindings/InputDevice";
import type { InstalledModel } from "./bindings/InstalledModel";
import type { Level } from "./bindings/Level";
import type { LinkSupport } from "./bindings/LinkSupport";
import type { Machine } from "./bindings/Machine";
import type { Platform } from "./bindings/Platform";
import type { ModelInfo } from "./bindings/ModelInfo";
import type { ModelSpec } from "./bindings/ModelSpec";
import type { Progress } from "./bindings/Progress";
import type { ProviderId } from "./bindings/ProviderId";
import type { Recommendation } from "./bindings/Recommendation";
import type { ArchiveStatus } from "./bindings/ArchiveStatus";
import type { ArchiveAnswer } from "./bindings/ArchiveAnswer";
import type { Citation } from "./bindings/Citation";
import type { ExportFormat } from "./bindings/ExportFormat";
import type { IndexProgress } from "./bindings/IndexProgress";
import type { PlayerOpen } from "./bindings/PlayerOpen";
import type { Recording } from "./bindings/Recording";
import type { RecordingDetail } from "./bindings/RecordingDetail";
import type { SearchHit } from "./bindings/SearchHit";
import type { CatalogueEntry } from "./bindings/CatalogueEntry";
import type { PullProgress } from "./bindings/PullProgress";
import type { SecretBackend } from "./bindings/SecretBackend";
import type { Suggestion } from "./bindings/Suggestion";
import type { Segment } from "./bindings/Segment";
import type { SourceProfile } from "./bindings/SourceProfile";
import type { Summary } from "./bindings/Summary";
import type { Turn } from "./bindings/Turn";
import type { Template } from "./bindings/Template";
import type { Transcript } from "./bindings/Transcript";

export type {
  AppSettings,
  ArchiveAnswer,
  ArchiveStatus,
  Citation,
  ExportFormat,
  IndexProgress,
  PlayerOpen,
  CatalogueEntry,
  PullProgress,
  Suggestion,
  Capabilities,
  CapabilityNote,
  InputDevice,
  InstalledModel,
  Level,
  LinkSupport,
  Machine,
  ModelInfo,
  Platform,
  ModelSpec,
  Progress,
  ProviderId,
  Recommendation,
  Recording,
  RecordingDetail,
  SearchHit,
  SecretBackend,
  Segment,
  SourceProfile,
  Summary,
  Template,
  Transcript,
  Turn,
};


export interface SourceAvailability {
  deviceId: string;
  available: boolean;
}


export interface SystemAudioDevice {
  id: string;
  name: string;
}


export interface AppPaths {
  configDir: string;
  dataDir: string;
  recordingsDir: string;
  modelsDir: string;
  logsDir: string;
  recordingsBytes: number;
  modelsBytes: number;
}


export interface HotkeyStatus {
  accelerator: string;
  display: string;
  registered: boolean;
  problem: string | null;
}

export interface RecordingStatus {
  active: boolean;
  paused: boolean;
  durationMs: number;
  level: Level;
  problem: string | null;


  rewindMs: number;
}


export interface LiveTranscript {
  recordingId: string;
  text: string;
  working: boolean;
}

export interface StageProgress {
  recordingId: string;
  stage: string;
  percent: number;
  step: number | null;
  steps: number | null;
  onThisMachine: boolean;
}


export function stageLabel(progress: StageProgress): string {
  const name =
    progress.stage === "importing"
      ? "Importing"
      : progress.stage === "transcribing"
        ? "Transcribing"
        : "Summarising";
  const step =
    progress.step && progress.steps && progress.steps > 1
      ? ` ${progress.step}/${progress.steps}`
      : "";
  return `${name}${step}`;
}

export const SLOWDOWN_NOTE =
  "Running on this computer — other things may feel slower until it finishes.";


export interface CommandError {
  code: string;
  message: string;
  remedy: string;
  retryable: boolean;
}

export interface PipelineFailure {
  recordingId: string;
  stage: string;
  error: CommandError;
}

export function toCommandError(error: unknown): CommandError {
  if (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    "message" in error &&
    "remedy" in error
  ) {
    return error as CommandError;
  }
  return {
    code: "unexpected",
    message: String(error),
    remedy: "If this keeps happening, report it with the log file — About says where it is.",
    retryable: false,
  };
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw toCommandError(error);
  }
}

export const ipc = {

  getSettings: () => call<AppSettings>("get_settings"),
  saveSettings: (settings: AppSettings) => call<AppSettings>("save_settings", { settings }),
  getTemplates: () => call<Template[]>("get_templates"),
  deriveTemplate: (fromId: string, name: string) =>
    call<Template>("derive_template", { fromId, name }),


  getCapabilities: () => call<Capabilities>("get_capabilities"),
  scanMachine: () => call<Machine>("scan_machine"),
  recommendModel: () => call<Recommendation>("recommend_model"),
  getPaths: () => call<AppPaths>("get_paths"),
  appVersion: () => call<string>("app_version"),
  gpuSupport: () => call<boolean>("gpu_support"),


  validateHotkey: (accelerator: string) => call<string>("validate_hotkey", { accelerator }),
  getHotkeyStatus: () => call<HotkeyStatus>("get_hotkey_status"),


  listModels: (provider: ProviderId) => call<ModelInfo[]>("list_models", { provider }),
  setApiKey: (provider: ProviderId, key: string) => call<void>("set_api_key", { provider, key }),
  hasApiKey: (provider: ProviderId) => call<boolean>("has_api_key", { provider }),
  getSecretBackend: () => call<SecretBackend>("get_secret_backend"),


  listCatalogue: () => call<CatalogueEntry[]>("list_catalogue"),
  suggestLlm: () => call<Suggestion>("suggest_llm"),
  llmAcceleration: () => call<boolean | null>("llm_acceleration"),
  installLlm: (modelId: string) => call<void>("install_llm", { modelId }),
  cancelLlmInstall: () => call<void>("cancel_llm_install"),

  listWhisperModels: () => call<ModelSpec[]>("list_whisper_models"),
  installedModels: () => call<InstalledModel[]>("installed_models"),
  downloadModel: (modelId: string) => call<void>("download_model", { modelId }),
  cancelModelDownload: () => call<void>("cancel_model_download"),
  removeModel: (modelId: string) => call<boolean>("remove_model", { modelId }),


  listInputDevices: () => call<InputDevice[]>("list_input_devices"),
  listSystemAudio: () => call<SystemAudioDevice[]>("list_system_audio"),
  checkSources: () => call<SourceAvailability[]>("check_sources"),
  startRecording: () => call<string>("start_recording"),
  stopRecording: () => call<void>("stop_recording"),
  toggleRecording: () => call<void>("toggle_recording"),
  pauseRecording: () => call<void>("pause_recording"),
  resumeRecording: () => call<void>("resume_recording"),
  recordingStatus: () => call<RecordingStatus>("recording_status"),
  cancelProcessing: () => call<void>("cancel_processing"),


  importFile: (path: string) => call<string>("import_file", { path }),
  importLink: (url: string) => call<string>("import_link", { url }),
  linkSupport: () => call<LinkSupport>("link_support"),
  importableExtensions: () => call<string[]>("importable_extensions"),


  audioUrl: (id: string) => call<string | null>("audio_url", { id }),
  audioPeaks: (id: string, buckets: number) => call<number[]>("audio_peaks", { id, buckets }),


  listRecordings: (limit: number) => call<Recording[]>("list_recordings", { limit }),
  searchRecordings: (query: string, limit: number) =>
    call<SearchHit[]>("search_recordings", { query, limit }),
  getRecording: (id: string) => call<RecordingDetail | null>("get_recording", { id }),
  renameRecording: (id: string, title: string) => call<void>("rename_recording", { id, title }),
  deleteRecording: (id: string) => call<void>("delete_recording", { id }),
  deleteAllRecordings: () => call<number>("delete_all_recordings"),
  recordingMarkdown: (id: string) => call<string>("recording_markdown", { id }),
  recordingSubtitles: (id: string, vtt: boolean) =>
    call<string>("recording_subtitles", { id, vtt }),


  exportRecording: (id: string, path: string, format: ExportFormat) =>
    call<void>("export_recording", { id, path, format }),
  ask: (id: string, history: Turn[], question: string) =>
    call<string>("ask", { id, history, question }),
  resummarise: (id: string) => call<void>("resummarise", { id }),
  retranscribe: (id: string) => call<void>("retranscribe", { id }),
  editTranscriptLine: (id: string, index: number, text: string) =>
    call<void>("edit_transcript_line", { id, index, text }),
  renameSpeaker: (id: string, from: string, to: string) =>
    call<number>("rename_speaker", { id, from, to }),


  setTags: (id: string, tags: string[]) => call<string[]>("set_tags", { id, tags }),
  listTags: () => call<[string, number][]>("list_tags"),


  openPlayer: (id: string, atMs?: number) => call<void>("open_player", { id, atMs }),
  playerRecording: () => call<PlayerOpen | null>("player_recording"),


  archiveStatus: () => call<ArchiveStatus>("archive_status"),
  indexArchive: () => call<number>("index_archive"),
  askArchive: (question: string) => call<ArchiveAnswer>("ask_archive", { question }),


  showMainWindow: () => call<void>("show_main_window"),
  setAutostart: (enabled: boolean) => call<void>("set_autostart", { enabled }),
  isAutostartEnabled: () => call<boolean>("is_autostart_enabled"),
} as const;

export const EVENTS = {
  recordingStarted: "zscribe://recording-started",
  recordingLevel: "zscribe://recording-level",
  recordingStopped: "zscribe://recording-stopped",
  transcribeProgress: "zscribe://transcribe-progress",
  summarizeProgress: "zscribe://summarize-progress",
  recordingReady: "zscribe://recording-ready",
  pipelineFailed: "zscribe://pipeline-failed",
  modelProgress: "zscribe://model-progress",
  llmProgress: "zscribe://llm-progress",
  liveTranscript: "zscribe://live-transcript",
  settingsChanged: "zscribe://settings-changed",
  playerOpen: "zscribe://player-open",
  indexProgress: "zscribe://index-progress",
  playerHidden: "zscribe://player-hidden",
} as const;

export function on<T>(event: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  return listen<T>(event, (message) => handler(message.payload));
}
