

import { mockConvertFileSrc, mockIPC, mockWindows } from "@tauri-apps/api/mocks";

import type { AppSettings } from "@/lib/bindings/AppSettings";
import type { Capabilities } from "@/lib/bindings/Capabilities";
import type { CatalogueEntry } from "@/lib/bindings/CatalogueEntry";
import type { InputDevice } from "@/lib/bindings/InputDevice";
import type { InstalledModel } from "@/lib/bindings/InstalledModel";
import type { Machine } from "@/lib/bindings/Machine";
import type { ModelSpec } from "@/lib/bindings/ModelSpec";
import type { Recommendation } from "@/lib/bindings/Recommendation";
import type { Recording } from "@/lib/bindings/Recording";
import type { RecordingDetail } from "@/lib/bindings/RecordingDetail";
import type { Suggestion } from "@/lib/bindings/Suggestion";

const params = new URLSearchParams(location.search);
const scenario = params.get("state") ?? "idle";
const flag = (name: string) => params.get(name) === "1" || params.has(name);


const now = Math.floor(Date.now() / 1000);

const SETTINGS: AppSettings = {
  schemaVersion: 1,
  hotkey: "Ctrl+Alt+R",
  templateId: "meeting",
  summaryLanguage: "auto",
  customTemplates: [
    {
      id: "custom-lyrics-11bc15bb",
      name: "Lyrics analysis",
      description: "Explains what a song is about and where it stays ambiguous.",
      instructions: "## Summary\n\nThree or four sentences on what the song is about.",
    },
  ],
  activeProvider: "ollama",
  providers: [
    { id: "ollama", baseUrl: null, model: "qwen2.5:7b" },
    { id: "gemini", baseUrl: null, model: "gemini-2.5-flash" },
    { id: "openAiCompatible", baseUrl: null, model: "gpt-4o-mini" },
  ],
  recording: {
    source: "microphone",
    inputDevice: null,
    sources: flag("multi")
      ? [
          {
            deviceId: "alsa:front:CARD=microphone,DEV=0",
            name: "Etienne",
            kind: "microphone",
            enabled: true,
          },
          {
            deviceId: "alsa:front:CARD=Generic,DEV=0",
            name: "Max Kruger",
            kind: "microphone",
            enabled: true,
          },
          {
            deviceId: "system:alsa_output.pci-0000_0c_00.4.analog-stereo.monitor",
            name: "Zoom call",
            kind: "systemAudio",
            enabled: true,
          },
        ]
      : [
          {
            deviceId: "alsa:front:CARD=microphone,DEV=0",
            name: "Etienne",
            kind: "microphone",
            enabled: true,
          },
        ],
    announceTone: true,
    consentNote: false,
    keepAudio: true,
    rewindSeconds: 0,
  },
  archive: { embeddingModel: "nomic-embed-text" },
  privacy: { redactContacts: true, redactSpeakers: false, redactTerms: [] },
  folders: { watch: null, notes: null },
  transcription: {
    modelId: "large-v3-turbo",
    language: "auto",
    useGpu: true,
    timestamps: true,
    detectSpeakers: true,
    liveTranscript: flag("live") || scenario === "live",
  },
  system: {
    startWithOs: false,
    startMinimized: false,
    minimizeToTray: true,
    showNotifications: true,
  },
  appearance: {
    theme: (params.get("theme") as AppSettings["appearance"]["theme"]) ?? "dark",
    opacity: 100,
    recorderOpacity: 100,
  },
  sidebar: { categories: [] },
  consentAcknowledged: !flag("consent"),
};


const SIDEBAR_SECTIONS = [
  "library",
  "ask",
  "hotkeys",
  "recording",
  "transcription",
  "templates",
  "providers",
  "appearance",
  "storage",
  "system",
  "about",
];

function normalizeSidebar(layout: AppSettings["sidebar"]): AppSettings["sidebar"] {
  const categories = layout.categories.map((category) => ({ ...category, items: [...category.items] }));
  const seen: string[] = [];

  for (const category of categories) {
    category.items = category.items.filter(
      (item) => SIDEBAR_SECTIONS.includes(item) && !seen.includes(item),
    );
    seen.push(...category.items);
  }

  if (categories.length === 0) {
    categories.push(
      { id: "recordings", name: "Recordings", collapsed: false, items: ["library", "ask"] },
      { id: "capture", name: "Capture", collapsed: false, items: ["hotkeys", "recording"] },
      {
        id: "understanding",
        name: "Understanding",
        collapsed: false,
        items: ["transcription", "templates", "providers"],
      },
      {
        id: "application",
        name: "Application",
        collapsed: false,
        items: ["appearance", "storage", "system", "about"],
      },
    );
    return { categories };
  }

  const missing = SIDEBAR_SECTIONS.filter((section) => !seen.includes(section));
  categories[0]!.items.push(...missing);
  return { categories };
}

SETTINGS.sidebar = normalizeSidebar(SETTINGS.sidebar);

const TRANSCRIPT_TEXT = [
  "Right, let's get started — I think everyone's here.",
  "The main thing today is the release. We said the fifteenth, and I want to be honest about whether that still holds.",
  "It holds if we cut the export formats down to Markdown. PDF is where all the remaining work is.",
  "That's fine by me. Nobody has asked for PDF yet, and Markdown is what people paste into their notes anyway.",
  "Then let's do that. Max, can you take the release notes?",
  "Yes. I'll draft them Thursday and send them round before they go out.",
  "One more thing — the installer on Windows still needs the Vulkan runtime, and we should say so on the download page.",
  "I'll add a line to it today.",
  "Good. Anything else? No? Then we're done, thanks everyone.",
];

const SPEAKERS = ["Etienne", "Etienne", "Max Kruger", "Etienne", "Etienne", "Max Kruger", "Etienne", "Max Kruger", "Etienne"];

function makeDetail(recording: Recording): RecordingDetail {
  if (!recording.hasTranscript) return { recording, transcript: null, summary: null };

  const transcript = {
    language: "en",
    model: "large-v3-turbo",
    segments: TRANSCRIPT_TEXT.map((text, index) => ({
      startMs: index * 7400,
      endMs: index * 7400 + 6900,
      text,
      speaker: SETTINGS.recording.sources.length > 1 ? (SPEAKERS[index] ?? null) : null,
    })),
  };

  if (!recording.hasSummary) return { recording, transcript, summary: null };

  return {
    recording,
    transcript,
    summary: {
      provider: "ollama",
      model: "qwen2.5:7b",
      templateId: "meeting",
      bodyMd: [
        "## Decisions",
        "",
        "- The release stays on the fifteenth.",
        "- Export ships as Markdown only; PDF is dropped from this release.",
        "- The download page will state that the Windows installer needs the Vulkan runtime.",
        "",
        "## Discussion",
        "",
        "The date was only at risk because of the export formats. Cutting PDF removes the remaining work, and nobody has asked for it — Markdown is what people paste into their notes.",
        "",
        "| Item | Owner | When |",
        "| --- | --- | --- |",
        "| Release notes | Max Kruger | Thursday |",
        "| Vulkan note on the download page | Max Kruger | Today |",
        "",
        "## Open questions",
        "",
        "- Nothing was raised that stayed unanswered.",
        "",
        "> Max: \"If PDF slips again we should say so on the download page rather than in the",
        "> changelog nobody reads.\"",
        "",
        "### Follow-up",
        "",
        "1. Cut `--features pdf` from the release build",
        "   - and drop the checklist line that mentions it",
        "2. Publish the notes",
        "",
        "- [x] Release date confirmed",
        "- [ ] Download page updated",
        "",
        "The build flag to remove:",
        "",
        "```bash",
        "pnpm app:build -- --no-default-features --features vulkan",
        "```",
        "",
        "```rust",
        "// and the export arm that goes with it",
        "match format {",
        "    Format::Markdown => write_markdown(&summary)?,",
        "    Format::Pdf => unreachable!(\"dropped for this release\"),",
        "}",
        "```",
        "",
        "Details are on the [release page](https://example.com/releases/0.1.0). ~~PDF~~ Markdown only.",
      ].join("\n"),
      actionItems: [
        { task: "Draft the release notes and circulate them before publishing", owner: "Max Kruger", due: "Thursday" },
        { task: "Add the Vulkan runtime note to the download page", owner: "Max Kruger", due: "Today" },
        { task: "Remove PDF export from the release checklist", owner: null, due: null },
      ],
      usage: { input: 2130, output: 486 },
      elapsedMs: 8420,
      redacted: 0,
    },
  };
}


const SEEDED_TAGS: Record<string, string[]> = {
  a1: ["release", "team"],
  a2: ["team"],
};

const RECORDINGS: Recording[] = flag("empty")
  ? []
  : [
      {
        id: "a1",
        startedAt: now - 60 * 42,
        durationMs: 18 * 60 * 1000 + 7000,
        source: "Etienne",
        templateId: "meeting",
        title: "Release planning — export formats and the fifteenth",
        audioPath: "/home/e/.local/share/zscribe/recordings/a1.wav",
        hasTranscript: true,
        hasSummary: true,
        tags: [],
      },
      {
        id: "a2",
        startedAt: now - 60 * 60 * 5,
        durationMs: 6 * 60 * 1000 + 1500,
        source: "Etienne",
        templateId: "self-note",
        title: "Note to self about the installer",
        audioPath: "/home/e/.local/share/zscribe/recordings/a2.wav",
        hasTranscript: true,
        hasSummary: true,
        tags: [],
      },
      {
        id: "a3",
        startedAt: now - 60 * 60 * 27,
        durationMs: 51 * 60 * 1000,
        source: "3 sources",
        templateId: "meeting",
        title: "Weekly sync with the design team",
        audioPath: null,
        hasTranscript: true,
        hasSummary: true,
        tags: [],
      },
      {
        id: "a4",
        startedAt: now - 60 * 60 * 50,
        durationMs: 2 * 60 * 1000 + 12000,
        source: "Etienne",
        templateId: "meeting",
        title: "Recording, 14 August 09:12",
        audioPath: "/home/e/.local/share/zscribe/recordings/a4.wav",
        hasTranscript: true,
        hasSummary: false,
        tags: [],
      },
      {
        id: "a5",
        startedAt: now - 60 * 60 * 74,
        durationMs: 41000,
        source: "Etienne",
        templateId: "meeting",
        title: "Recording, 13 August 08:40",
        audioPath: "/home/e/.local/share/zscribe/recordings/a5.wav",
        hasTranscript: false,
        hasSummary: false,
        tags: [],
      },
    ];

const MACHINE: Machine = {
  cpuModel: "AMD Ryzen 7 7840HS w/ Radeon 780M Graphics",
  cpuThreads: 16,
  totalRamMb: 31_500,
  availableRamMb: 19_240,
  freeDiskMb: 214_800,
  acceleration: "available",
  gpus: [
    { name: "AMD Radeon 780M Graphics (RADV PHOENIX)", vendor: "AMD", kind: "integrated", vramMb: 8192 },
  ],
};


const CAPABILITIES: Capabilities = {
  displayServer: (params.get("os") as Capabilities["displayServer"]) ?? "x11",
  hotkey: "os",
  notes: [],
};

if (CAPABILITIES.displayServer === "macOs") {
  MACHINE.cpuModel = "Apple M3 Pro";
  MACHINE.gpus = [
    { name: "Apple M3 Pro", vendor: "Apple", kind: "integrated", vramMb: MACHINE.totalRamMb },
  ];
}

const WHISPER_MODELS: ModelSpec[] = [
  { id: "tiny", label: "Tiny", bytes: 77_691_713, runtimeMb: 273, gpuSpeed: 32, cpuSpeed: 9, summary: "Fast and rough. Fine for a note to yourself.", sha256: "be07e048" },
  { id: "base", label: "Base", bytes: 147_951_465, runtimeMb: 388, gpuSpeed: 24, cpuSpeed: 6, summary: "A clear step up from Tiny at twice the size.", sha256: "60ed5bc3" },
  { id: "small", label: "Small", bytes: 487_601_967, runtimeMb: 852, gpuSpeed: 16, cpuSpeed: 3, summary: "Good on clear speech in a quiet room.", sha256: "1be3a9b2" },
  { id: "medium", label: "Medium", bytes: 1_533_763_059, runtimeMb: 2100, gpuSpeed: 8, cpuSpeed: 1.4, summary: "Handles accents and crosstalk noticeably better.", sha256: "6c14d5ad" },
  { id: "large-v3-turbo", label: "Large v3 Turbo", bytes: 1_624_555_275, runtimeMb: 1900, gpuSpeed: 11, cpuSpeed: 2, summary: "Large-model accuracy at close to Small's speed. The one to want.", sha256: "1fc70f77" },
];

const CATALOGUE: CatalogueEntry[] = [
  { id: "qwen2.5:0.5b", label: "Qwen 2.5 0.5B", megabytes: 398, overheadMb: 320, summary: "Tiny. Useful to prove the pipeline works, not to write a summary you would send." },
  { id: "llama3.2:3b", label: "Llama 3.2 3B", megabytes: 2000, overheadMb: 700, summary: "Reasonable summaries on a machine with modest memory." },
  { id: "qwen2.5:7b", label: "Qwen 2.5 7B", megabytes: 4700, overheadMb: 1200, summary: "The best balance for most machines. Follows a template closely." },
  { id: "gemma2:9b", label: "Gemma 2 9B", megabytes: 5400, overheadMb: 1400, summary: "Writes well. Wants 16 GB of memory to be comfortable." },
];

const INSTALLED: InstalledModel[] = [
  { id: "large-v3-turbo", installed: true, partial: false },
  { id: "tiny", installed: false, partial: false },
];

const DEVICES: InputDevice[] = flag("nodevices")
  ? []
  : [
      { id: "alsa:pulse", name: "System default (PulseAudio)", isDefault: true },
      { id: "alsa:front:CARD=microphone,DEV=0", name: "TONOR TC30 Microphone", isDefault: false },
      { id: "alsa:front:CARD=Generic,DEV=0", name: "Family 17h HD Audio Analog", isDefault: false },
    ];


type Emit = (event: string, payload: unknown) => void;
let emit: Emit = () => {};

const state = {
  active: scenario === "recording" || scenario === "paused" || scenario === "live" || scenario === "lost",
  paused: scenario === "paused",
  durationMs: scenario === "idle" ? 0 : 8 * 60 * 1000 + 23_000,
  level: { rms: 0.34, peak: 0.61 },
  problem: scenario === "lost" ? "Microphone lost" : null,


  rewindMs: flag("rewind") ? 47_000 : 0,
};


let ARCHIVE_INDEXED = -1;


let PLAYER_RECORDING: { id: string; atMs: number | null } = {
  id: params.get("player") ?? "a1",
  atMs: params.has("at") ? Number(params.get("at")) : null,
};

for (const recording of RECORDINGS) {
  recording.tags = SEEDED_TAGS[recording.id] ?? [];
}

function status() {
  return { ...state, level: state.paused ? { rms: 0, peak: 0 } : state.level };
}


mockWindows("main");


function playableWav(seconds: number): string {
  const rate = 16_000;
  const frames = rate * seconds;
  const bytes = new Uint8Array(44 + frames * 2);
  const view = new DataView(bytes.buffer);

  const ascii = (at: number, text: string) => {
    for (let i = 0; i < text.length; i++) view.setUint8(at + i, text.charCodeAt(i));
  };

  ascii(0, "RIFF");
  view.setUint32(4, 36 + frames * 2, true);
  ascii(8, "WAVEfmt ");
  view.setUint32(16, 16, true);
  view.setUint16(20, 1, true);
  view.setUint16(22, 1, true);
  view.setUint32(24, rate, true);
  view.setUint32(28, rate * 2, true);
  view.setUint16(32, 2, true);
  view.setUint16(34, 16, true);
  ascii(36, "data");
  view.setUint32(40, frames * 2, true);

  for (let frame = 0; frame < frames; frame++) {
    const t = frame / rate;


    const talking = t % 7.4 < 6.9 ? 1 : 0.02;
    const envelope = talking * (0.35 + 0.3 * Math.sin(t * 2.7));
    view.setInt16(44 + frame * 2, Math.sin(t * 2 * Math.PI * 180) * envelope * 32_000, true);
  }

  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return `data:audio/wav;base64,${btoa(binary)}`;
}

const AUDIO = playableWav(45);


mockConvertFileSrc("linux");


mockIPC(async (cmd, payload) => structuredClone(await handle(cmd, payload)) as never);

async function handle(cmd: string, payload: unknown) {
  const args = (payload ?? {}) as Record<string, unknown>;


  if (cmd.startsWith("plugin:window|") || cmd.startsWith("plugin:webview|")) {
    switch (cmd) {
      case "plugin:window|set_always_on_top":
      case "plugin:window|hide":
      case "plugin:window|show":
        console.info("[mock] window", cmd, args);
        return null;
      case "plugin:window|set_size": {


        const value = args.value as
          | { size?: { width: number; height: number }; Logical?: { width: number; height: number } }
          | undefined;
        const logical = value?.size ?? value?.Logical;
        if (logical) applyWindowSize(logical.width, logical.height);
        return null;
      }
      case "plugin:window|is_maximized":
        return flag("maximized");
      case "plugin:window|scale_factor":
        return 1;
      case "plugin:window|inner_size":
        return { width: window.innerWidth, height: window.innerHeight };
      default:
        return null;
    }
  }

  if (cmd.startsWith("plugin:event|")) {

    return null;
  }

  if (cmd === "plugin:dialog|save") {
    return "/home/e/Documents/release-planning.md";
  }
  if (cmd === "plugin:dialog|open") {
    if (flag("nofile")) return null;


    const options = (args.options ?? {}) as { directory?: boolean };
    return options.directory ? "/home/e/Vault/Meetings" : "/home/e/Videos/Team standup.mp4";
  }


  if (cmd.startsWith("plugin:") && !cmd.startsWith("plugin:event|")) {
    const known = ["plugin:dialog|", "plugin:opener|", "plugin:notification|", "plugin:window|"];
    if (!known.some((prefix) => cmd.startsWith(prefix))) {
      throw new Error(`${cmd} not allowed. Plugin not found`);
    }
  }
  if (cmd.startsWith("plugin:opener|")) {
    console.info("[mock] opener", args);
    return null;
  }

  switch (cmd) {
    case "get_settings":
      return SETTINGS;
    case "save_settings": {
      Object.assign(SETTINGS, args.settings);
      SETTINGS.sidebar = normalizeSidebar(SETTINGS.sidebar);
      emit("zscribe://settings-changed", SETTINGS);
      return SETTINGS;
    }
    case "get_templates":
      return [
        { id: "meeting", name: "Meeting", description: "Decisions, actions and open questions.", instructions: "" },
        { id: "self-note", name: "Note to self", description: "A short recap of what you said.", instructions: "" },
        { id: "interview", name: "Interview", description: "Questions, answers and quotable lines.", instructions: "" },
        { id: "lecture", name: "Lecture", description: "The argument, its structure and the terms used.", instructions: "" },
        ...SETTINGS.customTemplates,
      ];
    case "derive_template": {
      const created = {
        id: `custom-${crypto.randomUUID().slice(0, 8)}`,
        name: String(args.name ?? "Copy"),
        description: "A copy you can edit.",
        instructions: "## Summary\n\nWhat happened, in a few sentences.",
      };
      SETTINGS.customTemplates.push(created);
      return created;
    }

    case "get_capabilities":
      return CAPABILITIES;
    case "scan_machine":
      await sleep(500);
      return MACHINE;
    case "recommend_model":
      return {
        modelId: "large-v3-turbo",
        accelerated: true,
        headline: "AMD Radeon 780M with 8 GB — Large v3 Turbo will run about 11× faster than real time.",
        viable: ["tiny", "base", "small", "medium", "large-v3-turbo"],
        notes: [],
      } satisfies Recommendation;
    case "get_paths":
      return {
        configDir: "/home/e/.config/zscribe",
        dataDir: "/home/e/.local/share/zscribe",
        recordingsDir: "/home/e/.local/share/zscribe/recordings",
        modelsDir: "/home/e/.local/share/zscribe/models",
        logsDir: "/home/e/.local/share/zscribe/logs",
        recordingsBytes: 268_435_456,
        modelsBytes: 1_624_555_275,
      };
    case "app_version":
      return "0.1.0";
    case "gpu_support":
      return true;

    case "validate_hotkey":
      return String(args.accelerator ?? "");
    case "get_hotkey_status":
      return {
        accelerator: SETTINGS.hotkey,
        display: SETTINGS.hotkey.replace(/\+/g, " + "),
        registered: !flag("hotkeyfail"),
        problem: flag("hotkeyfail")
          ? "Another application already holds this combination."
          : null,
      };

    case "list_models":
      return [
        { id: "qwen2.5:7b", label: "Qwen 2.5 7B" },
        { id: "llama3.2:3b", label: "Llama 3.2 3B" },
      ];
    case "set_api_key":
      return null;
    case "has_api_key":
      return args.provider === "gemini";
    case "get_secret_backend":
      return "keychain";

    case "list_catalogue":
      return CATALOGUE;
    case "suggest_llm":
      return {
        modelId: "qwen2.5:7b",
        headline: "19 GB free — Qwen 2.5 7B is the best fit for this machine.",
        viable: CATALOGUE.map((entry) => entry.id),
      } satisfies Suggestion;
    case "llm_acceleration":
      return true;
    case "install_llm":
      void fakePull(String(args.modelId));
      return null;
    case "cancel_llm_install":
      return null;

    case "list_whisper_models":
      return WHISPER_MODELS;
    case "installed_models":
      return INSTALLED;
    case "download_model":
      void fakeDownload(String(args.modelId));
      return null;
    case "cancel_model_download":
      return null;
    case "remove_model": {
      const found = INSTALLED.find((m) => m.id === args.modelId);
      if (found) found.installed = false;
      return true;
    }

    case "list_input_devices":
      return DEVICES;
    case "list_system_audio":
      return [
        { id: "alsa_output.pci-0000_0c_00.4.analog-stereo.monitor", name: "FiiO K5 Pro Analog Stereo" },
        { id: "alsa_output.usb-Generic.analog-stereo.monitor", name: "Built-in Audio Analog Stereo" },
      ];
    case "check_sources":
      return SETTINGS.recording.sources.map((source) => ({
        deviceId: source.deviceId,
        available: !flag("unplugged"),
      }));

    case "start_recording":
      startRecording();
      return "new-recording";
    case "stop_recording":
    case "toggle_recording":
      if (state.active) stopRecording();
      else startRecording();
      return null;
    case "pause_recording":
      state.paused = true;
      emit("zscribe://recording-level", status());
      return null;
    case "resume_recording":
      state.paused = false;
      emit("zscribe://recording-level", status());
      return null;
    case "recording_status":
      return status();
    case "cancel_processing":
      return null;


    case "link_support": {
      const missing = flag("noytdlp");
      const platform = params.get("platform") ?? "linux";
      const windows = platform === "windows";


      const bin = windows
        ? "C:\\Users\\e\\AppData\\Roaming\\TheHolyOneZ\\ZScribe\\data\\bin"
        : platform === "macos"
          ? "/Users/e/Library/Application Support/dev.TheHolyOneZ.ZScribe/bin"
          : "/home/e/.local/share/zscribe/bin";
      const target = windows ? `${bin}\\yt-dlp.exe` : `${bin}/yt-dlp`;
      const asset =
        windows ? "yt-dlp.exe" : platform === "macos" ? "yt-dlp_macos" : "yt-dlp_linux";
      const install =
        windows ? "winget install yt-dlp" : platform === "macos"
          ? "brew install yt-dlp"
          : "sudo pacman -S yt-dlp";

      return {
        available: !missing,
        path: missing ? null : "/usr/bin/yt-dlp",
        version: missing ? null : flag("staleytdlp") ? "2026.05.28" : "2026.08.16.232411",
        versionAgeDays: missing ? null : flag("staleytdlp") ? 80 : 0,
        staleAfterDays: 30,
        installCommand: install,
        nightlyCommand: `${windows ? "curl.exe" : "curl"} -L --create-dirs -o "${target}" https://github.com/yt-dlp/yt-dlp-nightly-builds/releases/latest/download/${asset}${
          windows ? "" : ` && chmod +x "${target}"`
        }`,
        standaloneAsset: asset,
        toolsDir: bin,
        toolsPath: target,
        platform,
        jsRuntime: flag("nojs") ? null : "deno",
        jsRuntimeCommand: install.replace("yt-dlp", "deno"),
      };
    }
    case "export_recording":
      console.info("[mock] exported", args);
      return null;

    case "recording_subtitles": {
      const vtt = Boolean(args.vtt);
      const stamp = (ms: number) => {
        const pad = (n: number, w = 2) => String(n).padStart(w, "0");
        return `${pad(Math.floor(ms / 3600000))}:${pad(Math.floor((ms % 3600000) / 60000))}:${pad(
          Math.floor((ms % 60000) / 1000),
        )}${vtt ? "." : ","}${pad(ms % 1000, 3)}`;
      };
      const cues = TRANSCRIPT_TEXT.map((text, index) => {
        const start = index * 7400;
        const head = vtt ? "" : `${index + 1}\n`;
        return `${head}${stamp(start)} --> ${stamp(start + 6900)}\n${text}\n`;
      });
      return (vtt ? "WEBVTT\n\n" : "") + cues.join("\n");
    }
    case "set_tags": {
      const id = String(args.id ?? "");
      const wanted = (args.tags as string[]).map((t) => t.trim()).filter(Boolean);

      const unique: string[] = [];
      for (const tag of wanted) {
        if (!unique.some((existing) => existing.toLowerCase() === tag.toLowerCase())) {
          unique.push(tag);
        }
      }
      unique.sort();

      const recording = RECORDINGS.find((r) => r.id === id);
      if (recording) recording.tags = unique;
      return unique;
    }
    case "list_tags": {
      const counts = new Map<string, number>();
      for (const recording of RECORDINGS) {
        for (const tag of recording.tags) counts.set(tag, (counts.get(tag) ?? 0) + 1);
      }
      return [...counts.entries()].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]));
    }
    case "rename_speaker": {
      const from = String(args.from ?? "");
      const to = String(args.to ?? "");
      let renamed = 0;
      for (let i = 0; i < SPEAKERS.length; i++) {
        if (SPEAKERS[i] === from) {
          SPEAKERS[i] = to;
          renamed++;
        }
      }
      return renamed;
    }
    case "edit_transcript_line": {
      const index = Number(args.index);
      if (index >= 0 && index < TRANSCRIPT_TEXT.length) {
        TRANSCRIPT_TEXT[index] = String(args.text ?? "");
      }
      return null;
    }
    case "audio_url":
      return AUDIO;
    case "audio_peaks": {

      const buckets = Number(args.buckets ?? 480);
      const seconds = 45;
      return Array.from({ length: buckets }, (_, index) => {
        const t = (index / buckets) * seconds;
        if (t % 7.4 > 6.9) return 4;
        return Math.round((0.35 + 0.3 * Math.sin(t * 2.7)) * 255);
      });
    }
    case "importable_extensions":
      return ["wav", "mp3", "m4a", "mp4", "aac", "flac", "ogg", "mkv", "aiff"];
    case "import_file":
    case "import_link": {


      await sleep(400);

      if (flag("importfails")) {
        throw {
          code: "no_audio_track",
          message: "Team standup.mp4 has no audio in it",
          remedy:
            "Only the sound is used, so a video is fine — but this file has no audio track at all.",
          retryable: false,
        };
      }

      const title =
        cmd === "import_link"
          ? "Ask not what your country can do for you"
          : String(args.path ?? "").replace(/^.*\//, "").replace(/\.[^.]+$/, "");

      const imported: Recording = {
        id: `imported-${RECORDINGS.length}`,
        startedAt: Math.floor(Date.now() / 1000),
        durationMs: 11_000,
        source: cmd === "import_link" ? "Imported from Youtube" : "Imported mp4",
        templateId: "meeting",
        title,
        audioPath: "/home/e/.local/share/zscribe/recordings/imported.wav",
        hasTranscript: true,
        hasSummary: true,
        tags: [],
      };

      RECORDINGS.unshift(imported);


      emit("zscribe://recording-started", imported.id);
      for (let percent = 0; percent <= 100; percent += 10) {
        emit("zscribe://transcribe-progress", {
          recordingId: imported.id,
          stage: "importing",
          percent,
          step: null,
          steps: null,
          onThisMachine: true,
        });
        await sleep(140);
      }

      void fakeStage("transcribing", imported.id);
      return imported.id;
    }

    case "list_recordings":
      return RECORDINGS;


    case "search_recordings": {
      const needle = String(args.query ?? "").trim().toLowerCase();
      if (!needle) return [];

      const hits = [];
      for (const recording of RECORDINGS) {
        const detail = makeDetail(recording);
        const haystacks = [
          recording.title,
          detail.transcript?.segments.map((segment) => segment.text).join(" ") ?? "",
          detail.summary?.bodyMd ?? "",
        ];

        for (const text of haystacks) {
          const at = text.toLowerCase().indexOf(needle);
          if (at < 0) continue;

          const from = Math.max(0, at - 40);
          const to = Math.min(text.length, at + needle.length + 60);
          const snippet =
            (from > 0 ? "…" : "") +
            text.slice(from, at) +
            "\ue000" +
            text.slice(at, at + needle.length) +
            "\ue001" +
            text.slice(at + needle.length, to) +
            (to < text.length ? "…" : "");

          hits.push({ recording, snippet });
          break;
        }
      }
      return hits;
    }
    case "get_recording": {
      const found = RECORDINGS.find((r) => r.id === args.id);
      return found ? makeDetail(found) : null;
    }
    case "rename_recording": {
      const found = RECORDINGS.find((r) => r.id === args.id);
      if (found) found.title = String(args.title);
      return null;
    }
    case "delete_recording": {
      const index = RECORDINGS.findIndex((r) => r.id === args.id);
      if (index !== -1) RECORDINGS.splice(index, 1);
      return null;
    }
    case "delete_all_recordings": {
      const count = RECORDINGS.length;
      RECORDINGS.length = 0;
      return count;
    }
    case "recording_markdown": {
      const found = RECORDINGS.find((r) => r.id === args.id);
      const detail = found ? makeDetail(found) : null;
      return `# ${found?.title ?? "Recording"}\n\n${detail?.summary?.bodyMd ?? ""}`;
    }
    case "ask":
      await sleep(900);
      return [
        "The release **stays on the fifteenth**. The only thing that changed is export:",
        "Markdown ships, PDF was dropped so the date could hold.",
        "",
        "| What | Decided |",
        "| --- | --- |",
        "| Date | 15th, unchanged |",
        "| Export | Markdown only |",
        "",
        "Max is drafting the notes on Thursday. The flag to remove is `--features pdf`:",
        "",
        "```toml",
        "[features]",
        "default = [\"vulkan\"]",
        "```",
        "",
        "> The transcript does not say who signs off the download page.",
      ].join("\n");
    case "resummarise":
      void fakeStage("summarising", String(args.id));
      return null;
    case "retranscribe":
      void fakeStage("transcribing", String(args.id));
      return null;


    case "open_player":
      PLAYER_RECORDING = { id: String(args.id ?? ""), atMs: (args.atMs as number | undefined) ?? null };
      emit("zscribe://player-open", PLAYER_RECORDING);
      console.info("[mock] the player window would open", PLAYER_RECORDING);
      return null;
    case "player_recording":
      return PLAYER_RECORDING;


    case "archive_status": {
      const transcribed = RECORDINGS.filter((r) => r.hasTranscript).length;
      if (ARCHIVE_INDEXED < 0) {
        ARCHIVE_INDEXED = flag("unindexed") ? Math.max(0, transcribed - 2) : transcribed;
      }
      return {
        embeddingModel: "nomic-embed-text",
        transcribed,
        indexed: ARCHIVE_INDEXED,
        passages: ARCHIVE_INDEXED * 7,
        ollamaReady: !flag("noollama"),
        modelReady: !flag("noembed"),
      };
    }
    case "index_archive": {
      const transcribed = RECORDINGS.filter((r) => r.hasTranscript).length;
      for (let done = 0; done <= transcribed; done++) {
        emit("zscribe://index-progress", {
          done,
          total: transcribed,
          title: RECORDINGS[done]?.title ?? "",
        });
        await sleep(280);
      }
      ARCHIVE_INDEXED = transcribed;
      return transcribed;
    }
    case "ask_archive": {
      await sleep(1200);
      const asked = String(args.question ?? "");
      return {
        text: [
          `**The fifteenth still holds.** In *Release planning* the date was confirmed once export`,
          "was cut back to Markdown only.",
          "",
          "- *Release planning* — PDF dropped so the date could hold",
          "- *Weekly sync with the design team* — nobody had asked for PDF",
          "",
          `> None of your recordings say anything about ${asked.split(" ").slice(-1)[0] ?? "that"}`,
          "> beyond what is quoted here.",
        ].join("\n"),
        citations: RECORDINGS.filter((r) => r.hasTranscript)
          .slice(0, 3)
          .map((recording, index) => ({
            recordingId: recording.id,
            title: recording.title,
            startedAt: recording.startedAt,
            startMs: 7_400 * (index + 1),
            text: TRANSCRIPT_TEXT[index + 1] ?? "",
            score: 0.72 - index * 0.06,
          })),
      };
    }

    case "show_main_window":
      return null;
    case "set_autostart":
      return null;
    case "is_autostart_enabled":
      return false;

    default:
      console.warn("[mock] unhandled command", cmd, args);
      return null;
  }
}


{
  const listeners = new Map<string, Set<number>>();
  const inner = window.__TAURI_INTERNALS__.invoke;

  window.__TAURI_INTERNALS__.invoke = (cmd, args, options) => {
    const payload = (args ?? {}) as { event?: string; handler?: number; eventId?: number };

    if (cmd === "plugin:event|listen" && payload.event && payload.handler !== undefined) {
      const set = listeners.get(payload.event) ?? new Set<number>();
      set.add(payload.handler);
      listeners.set(payload.event, set);


      scheduleScenario();
      return Promise.resolve(payload.handler as unknown);
    }

    if (cmd === "plugin:event|unlisten") {
      for (const set of listeners.values()) set.delete(payload.eventId as number);
      return Promise.resolve(null);
    }

    return inner(cmd, args, options);
  };

  emit = (event, data) => {
    for (const id of listeners.get(event) ?? []) {
      window.__TAURI_INTERNALS__.runCallback?.(id, { event, id, payload: data });
    }
  };
}


const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

let ticker: number | undefined;


function speechLevel(elapsedMs: number) {
  const phrase = Math.sin(elapsedMs / 1400) * 0.5 + 0.5;
  const syllable = Math.sin(elapsedMs / 190) * 0.5 + 0.5;
  const gap = phrase < 0.22 ? 0 : 1;

  const rms = gap * (0.04 + phrase * 0.5 * (0.45 + syllable * 0.55) + Math.random() * 0.05);
  return { rms, peak: Math.min(1, rms * 1.6) };
}

function startRecording() {
  state.active = true;
  state.paused = false;
  state.durationMs = 0;
  emit("zscribe://recording-started", "new-recording");

  const started = Date.now();
  ticker = window.setInterval(() => {
    if (state.paused) return;
    state.durationMs = Date.now() - started;
    state.level = speechLevel(state.durationMs);
    emit("zscribe://recording-level", status());
  }, 120);
}

function stopRecording() {
  state.active = false;
  state.paused = false;
  if (ticker) window.clearInterval(ticker);
  emit("zscribe://recording-stopped", "new-recording");
  void fakeStage("transcribing", "new-recording");
}

async function fakeStage(stage: string, recordingId: string) {
  const event =
    stage === "transcribing" ? "zscribe://transcribe-progress" : "zscribe://summarize-progress";

  for (let percent = 0; percent <= 100; percent += 4) {
    emit(event, { recordingId, stage, percent, step: null, steps: null, onThisMachine: true });
    await sleep(120);
  }

  if (stage === "transcribing") {
    await fakeStage("summarising", recordingId);
    return;
  }
  emit("zscribe://recording-ready", recordingId);
}


async function fakePull(modelId: string) {
  const stages = ["pulling manifest", "verifying sha256 digest", "writing manifest"];
  const send = (progress: unknown) => emit("zscribe://llm-progress", [modelId, progress]);

  send({ downloadedBytes: 0, totalBytes: 0, percent: 0, stage: stages[0]! });
  await sleep(700);

  const total = (CATALOGUE.find((entry) => entry.id === modelId)?.megabytes ?? 1000) * 1_000_000;
  for (let percent = 0; percent <= 100; percent += 2) {
    send({
      downloadedBytes: Math.round((total * percent) / 100),
      totalBytes: total,
      percent,
      stage: `pulling ${modelId}`,
    });
    await sleep(90);
  }

  for (const stage of stages.slice(1)) {
    send({ downloadedBytes: total, totalBytes: 0, percent: 100, stage });
    await sleep(600);
  }
  send(null);
}

async function fakeDownload(modelId: string) {
  const total = WHISPER_MODELS.find((model) => model.id === modelId)?.bytes ?? 1_624_555_275;
  const send = (progress: unknown) => emit("zscribe://model-progress", [modelId, progress]);

  for (let percent = 0; percent <= 100; percent += 2) {
    send({
      downloadedBytes: Math.round((total * percent) / 100),
      totalBytes: total,
      percent,
      verifying: false,
    });
    await sleep(80);
  }

  send({ downloadedBytes: total, totalBytes: total, percent: 100, verifying: true });
  await sleep(1200);

  const installed = INSTALLED.find((model) => model.id === modelId);
  if (installed) installed.installed = true;
  else INSTALLED.push({ id: modelId, installed: true, partial: false });

  send(null);
}


function applyWindowSize(width: number, height: number) {
  const frame = document.getElementById("frame") ?? document.getElementById("root");
  if (!frame) return;
  frame.style.width = `${width}px`;
  frame.style.height = `${height}px`;
  document.title = `ZScribe recorder — ${width}×${height}`;
  const readout = document.getElementById("size-readout");
  if (readout) readout.textContent = `${width} × ${height}`;
}


declare global {
  interface Window {
    __TAURI_INTERNALS__: {
      invoke: (cmd: string, args?: unknown, options?: unknown) => Promise<unknown>;
      runCallback?: (id: number, data: unknown) => void;
      [key: string]: unknown;
    };
    mock: typeof api;
  }
}

const api = {
  emit: (event: string, payload: unknown) => emit(event, payload),
  start: startRecording,
  stop: stopRecording,
  pause: () => {
    state.paused = true;
    emit("zscribe://recording-level", status());
  },
  resume: () => {
    state.paused = false;
    emit("zscribe://recording-level", status());
  },

  level: (rms: number, peak = rms) => {
    if (ticker) window.clearInterval(ticker);
    state.level = { rms, peak };
    emit("zscribe://recording-level", status());
  },
  duration: (ms: number) => {
    if (ticker) window.clearInterval(ticker);
    state.durationMs = ms;
    emit("zscribe://recording-level", status());
  },
  problem: (text: string | null) => {
    state.problem = text;
    emit("zscribe://recording-level", status());
  },
  working: (percent: number | null, stage = "transcribing") => {
    state.active = false;
    if (ticker) window.clearInterval(ticker);
    emit("zscribe://recording-stopped", "new-recording");
    if (percent !== null) {
      emit("zscribe://transcribe-progress", {
        recordingId: "new-recording",
        stage,
        percent,
        step: null,
        steps: null,
        onThisMachine: true,
      });
    }
  },
  transcript: (text: string, working = true) =>
    emit("zscribe://live-transcript", { recordingId: "new-recording", text, working }),
  fail: (message: string) =>
    emit("zscribe://pipeline-failed", {
      recordingId: "a1",
      stage: "summarising",
      error: {
        code: "provider_unreachable",
        message,
        remedy: "Start Ollama, then try again.",
        retryable: true,
      },
    }),
};

window.mock = api;

let scenarioTimer: number | undefined;


function scheduleScenario() {
  if (scenarioApplied) return;
  if (scenarioTimer) window.clearTimeout(scenarioTimer);
  scenarioTimer = window.setTimeout(applyScenario, 80);
}

let scenarioApplied = false;

function applyScenario() {
  if (scenarioApplied) return;
  scenarioApplied = true;

  if (state.active) {
    const started = Date.now() - state.durationMs;
    ticker = window.setInterval(() => {
      if (state.paused) return;
      state.durationMs = Date.now() - started;
      state.level = speechLevel(state.durationMs);
      emit("zscribe://recording-level", status());
    }, 120);
    emit("zscribe://recording-level", status());
  }

  if (scenario === "live") {
    api.transcript(TRANSCRIPT_TEXT.slice(0, 4).join(" "), true);
  }

  if (scenario === "working") api.working(37);
  if (scenario === "working-start") api.working(null);
  if (scenario === "failed") api.fail("Ollama is not running.");
}

export {};
