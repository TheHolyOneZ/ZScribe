import { useEffect, useState, type ComponentType } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  AudioLines,
  Boxes,
  HardDrive,
  Info,
  Keyboard,
  LayoutTemplate,
  Library,
  Mic,
  Monitor,
  Palette,
  Sparkles,
} from "lucide-react";

import { StatusDot } from "@/components/ui";
import { Sidebar, type SidebarEntry } from "@/components/ui/Sidebar";
import { ResizeEdges, Titlebar } from "@/components/ui/Titlebar";
import { useEta } from "@/lib/eta";
import { clock } from "@/lib/format";
import {
  EVENTS,
  ipc,
  on,
  type AppSettings,
  type PipelineFailure,
  type RecordingStatus,
  type StageProgress,
  SLOWDOWN_NOTE,
  stageLabel,
} from "@/lib/ipc";
import { useAppStore } from "@/store/useAppStore";

import { ConsentGate } from "./ConsentGate";
import { ImportButton } from "./ImportButton";
import { Library as LibraryView } from "./Library";
import { RecordButton } from "./RecordButton";
import { AboutPanel } from "./panels/AboutPanel";
import { AskPanel } from "./panels/AskPanel";
import { AppearancePanel } from "./panels/AppearancePanel";
import { HotkeysPanel } from "./panels/HotkeysPanel";
import { ProvidersPanel } from "./panels/ProvidersPanel";
import { RecordingPanel } from "./panels/RecordingPanel";
import { StoragePanel } from "./panels/StoragePanel";
import { SystemPanel } from "./panels/SystemPanel";
import { TemplatesPanel } from "./panels/TemplatesPanel";
import { TranscriptionPanel } from "./panels/TranscriptionPanel";


const SECTIONS = [
  { id: "library", label: "Library", icon: Library },
  { id: "ask", label: "Ask everything", icon: Sparkles },
  { id: "hotkeys", label: "Hotkeys", icon: Keyboard },
  { id: "recording", label: "Audio sources", icon: Mic },
  { id: "transcription", label: "Speech to text", icon: AudioLines },
  { id: "templates", label: "Summary styles", icon: LayoutTemplate },
  { id: "providers", label: "AI models", icon: Boxes },
  { id: "appearance", label: "Appearance", icon: Palette },
  { id: "storage", label: "Storage", icon: HardDrive },
  { id: "system", label: "Startup & window", icon: Monitor },
  { id: "about", label: "About", icon: Info },
] as const satisfies readonly SidebarEntry[];

type SectionId = (typeof SECTIONS)[number]["id"];

const PANELS: Record<SectionId, ComponentType> = {
  library: LibraryView,
  ask: AskPanel,
  hotkeys: HotkeysPanel,
  recording: RecordingPanel,
  transcription: TranscriptionPanel,
  templates: TemplatesPanel,
  providers: ProvidersPanel,
  appearance: AppearancePanel,
  storage: StoragePanel,
  system: SystemPanel,
  about: AboutPanel,
};

export function App() {
  const [section, setSection] = useState<SectionId>("library");
  const [maximized, setMaximized] = useState(false);

  const loading = useAppStore((state) => state.loading);
  const settings = useAppStore((state) => state.settings);
  const status = useAppStore((state) => state.status);
  const progress = useAppStore((state) => state.progress);
  const hydrate = useAppStore((state) => state.hydrate);
  const refreshRecordings = useAppStore((state) => state.refreshRecordings);
  const update = useAppStore((state) => state.update);
  const setStatus = useAppStore((state) => state.setStatus);
  const setProgress = useAppStore((state) => state.setProgress);
  const select = useAppStore((state) => state.select);

  useEffect(() => {
    void hydrate();
  }, [hydrate]);


  useEffect(() => {
    const listeners = [
      on<RecordingStatus>(EVENTS.recordingLevel, setStatus),
      on<string>(EVENTS.recordingStarted, (id) => {
        select(id);
        setSection("library");
        void refreshRecordings();
      }),
      on<string>(EVENTS.recordingStopped, () => {
        void ipc.recordingStatus().then(setStatus);
        void refreshRecordings();
      }),
      on<StageProgress>(EVENTS.transcribeProgress, setProgress),
      on<StageProgress>(EVENTS.summarizeProgress, setProgress),
      on<string>(EVENTS.recordingReady, () => {
        setProgress(null);
        void refreshRecordings();
      }),
      on<PipelineFailure>(EVENTS.pipelineFailed, () => setProgress(null)),
      on<AppSettings>(EVENTS.settingsChanged, () => void hydrate()),
    ];

    return () => {
      for (const listener of listeners) void listener.then((off) => off());
    };
  }, [hydrate, refreshRecordings, select, setProgress, setStatus]);

  useEffect(() => {
    const appWindow = getCurrentWindow();
    let cancelled = false;

    const sync = () => {
      void appWindow.isMaximized().then((value) => {
        if (!cancelled) setMaximized(value);
      });
    };
    sync();

    const unlisten = appWindow.onResized(sync);
    return () => {
      cancelled = true;
      void unlisten.then((off) => off());
    };
  }, []);

  useEffect(() => {
    const theme = settings?.appearance.theme ?? "system";
    const root = document.documentElement;

    if (theme !== "system") {
      root.setAttribute("data-theme", theme);
      return;
    }

    const query = window.matchMedia("(prefers-color-scheme: light)");
    const apply = () => root.setAttribute("data-theme", query.matches ? "light" : "dark");

    apply();
    query.addEventListener("change", apply);
    return () => query.removeEventListener("change", apply);
  }, [settings?.appearance.theme]);

  useEffect(() => {
    document.documentElement.style.opacity = String((settings?.appearance.opacity ?? 100) / 100);
  }, [settings?.appearance.opacity]);

  const Panel = PANELS[section];

  return (
    <>
      <ResizeEdges />
      <div className="window-shell flex flex-col" data-maximized={maximized}>
        <Titlebar>
          <ImportButton />
          <RecordButton />
        </Titlebar>

        <div className="flex min-h-0 flex-1">
          <Sidebar
            entries={SECTIONS as unknown as SidebarEntry<SectionId>[]}
            layout={settings?.sidebar ?? { categories: [] }}
            active={section}
            onSelect={setSection}
            onLayoutChange={(sidebar) => void update({ sidebar })}
            onResetLayout={() => void update({ sidebar: { categories: [] } })}
            footer={
              loading ? (
                <StatusDot tone="neutral">Starting…</StatusDot>
              ) : (
                <Footer status={status} progress={progress} />
              )
            }
          />

          <main className="min-w-0 flex-1">{loading ? null : <Panel />}</main>
        </div>
      </div>

      <ConsentGate />
    </>
  );
}


function Footer({
  status,
  progress,
}: {
  status: RecordingStatus;
  progress: StageProgress | null;
}) {
  const eta = useEta(
    progress?.percent ?? null,
    progress ? `${progress.stage}:${progress.recordingId}` : "",
  );

  if (status.active) {
    return (
      <div className="space-y-1">
        <StatusDot tone="danger">{status.paused ? "Paused" : "Recording"}</StatusDot>
        <p data-numeric className="text-2xs text-faint">
          {clock(status.durationMs)}
        </p>
      </div>
    );
  }

  if (progress) {
    return (
      <div className="space-y-1.5">
        <StatusDot tone="accent">
          {stageLabel(progress)}
          {eta ? <span className="text-faint"> · {eta}</span> : null}
        </StatusDot>
        <div className="h-1 overflow-hidden rounded-full bg-inset">
          <div
            className="h-full rounded-full bg-accent transition-[width] duration-base ease-out"
            style={{ width: `${progress.percent}%` }}
          />
        </div>
        {progress.onThisMachine ? (
          <p className="text-2xs leading-relaxed text-faint">{SLOWDOWN_NOTE}</p>
        ) : null}
      </div>
    );
  }


  if (status.rewindMs > 0) {
    return (
      <div className="space-y-1">
        <StatusDot tone="success">Ready</StatusDot>
        <p data-numeric className="text-2xs text-faint" title="The microphone is open so that starting a recording includes what was already being said. Nothing is written to disk.">
          Listening · {clock(status.rewindMs)} back
        </p>
      </div>
    );
  }

  return <StatusDot tone="success">Ready</StatusDot>;
}
