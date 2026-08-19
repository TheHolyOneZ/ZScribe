import { useCallback, useEffect, useState } from "react";
import { Check, Cpu, Download, HardDrive, RefreshCw, Trash2, X, Zap } from "lucide-react";

import {
  Button,
  Callout,
  ContextMenu,
  MenuItem,
  MenuLabel,
  MenuSeparator,
  Panel,
  Select,
  SettingGroup,
  SettingRow,
  Skeleton,
  Switch,
} from "@/components/ui";
import { cn } from "@/lib/cn";
import { bytes, gpuBackend } from "@/lib/format";
import {
  EVENTS,
  ipc,
  on,
  toCommandError,
  type CapabilityNote,
  type CommandError,
  type InstalledModel,
  type Machine,
  type ModelSpec,
  type Progress,
  type Recommendation,
} from "@/lib/ipc";
import { useAppStore } from "@/store/useAppStore";


const LANGUAGES = [
  { value: "auto", label: "Detect automatically" },
  { value: "en", label: "English" },
  { value: "de", label: "German" },
  { value: "fr", label: "French" },
  { value: "es", label: "Spanish" },
  { value: "it", label: "Italian" },
  { value: "nl", label: "Dutch" },
  { value: "pt", label: "Portuguese" },
  { value: "pl", label: "Polish" },
  { value: "ru", label: "Russian" },
  { value: "tr", label: "Turkish" },
  { value: "zh", label: "Chinese" },
  { value: "ja", label: "Japanese" },
];


export function TranscriptionPanel() {
  const settings = useAppStore((state) => state.settings);
  const update = useAppStore((state) => state.update);

  const [machine, setMachine] = useState<Machine | null>(null);
  const [recommendation, setRecommendation] = useState<Recommendation | null>(null);
  const [models, setModels] = useState<ModelSpec[]>([]);
  const [installed, setInstalled] = useState<InstalledModel[]>([]);
  const [scanning, setScanning] = useState(false);
  const [downloads, setDownloads] = useState<Record<string, Progress>>({});
  const [error, setError] = useState<CommandError | null>(null);

  const refreshInstalled = useCallback(async () => {
    setInstalled(await ipc.installedModels());
  }, []);

  const scan = useCallback(async () => {
    setScanning(true);
    setError(null);
    try {
      const [nextMachine, nextRecommendation] = await Promise.all([
        ipc.scanMachine(),
        ipc.recommendModel(),
      ]);
      setMachine(nextMachine);
      setRecommendation(nextRecommendation);
    } catch (raw) {
      setError(toCommandError(raw));
    } finally {
      setScanning(false);
    }
  }, []);

  useEffect(() => {
    void ipc.listWhisperModels().then(setModels);
    void refreshInstalled();
    void scan();
  }, [refreshInstalled, scan]);

  useEffect(() => {
    const unlisten = on<[string, Progress | null]>(EVENTS.modelProgress, ([id, progress]) => {
      setDownloads((current) => {
        const next = { ...current };
        if (progress) next[id] = progress;
        else delete next[id];
        return next;
      });

      if (!progress) void refreshInstalled();
    });

    return () => {
      void unlisten.then((off) => off());
    };
  }, [refreshInstalled]);

  const download = async (id: string) => {
    setError(null);
    try {
      await ipc.downloadModel(id);
    } catch (raw) {
      setError(toCommandError(raw));
    }
  };

  const remove = async (id: string) => {
    await ipc.removeModel(id);
    await refreshInstalled();
    if (settings?.transcription.modelId === id) await update({
      transcription: { ...settings.transcription, modelId: "" },
    });
  };

  const isInstalled = (id: string) => installed.some((m) => m.id === id && m.installed);

  return (
    <Panel
      title="Speech to text"
      description="Whisper runs on this machine. Your audio is never uploaded — that is not a setting, it is how it works."
      actions={
        <Button size="sm" onClick={() => void scan()} disabled={scanning}>
          <RefreshCw className={cn(scanning && "animate-spin")} />
          Scan again
        </Button>
      }
    >
      <SettingGroup
        title="This machine"
        description="Measured now, not guessed. Run the scan again after installing a graphics driver."
      >
        <div className="p-3.5">
          {scanning && !machine ? (
            <div className="space-y-2">
              <Skeleton className="h-4 w-3/4" />
              <Skeleton className="h-3 w-1/2" />
            </div>
          ) : machine ? (
            <MachineReport machine={machine} recommendation={recommendation} />
          ) : (
            <p className="text-sm text-muted">The scan could not run.</p>
          )}
        </div>
      </SettingGroup>

      {recommendation?.notes.length ? (
        <div className="mb-8 space-y-2">
          {recommendation.notes.map((note, index) => (
            <NoteCallout key={index} note={note} />
          ))}
        </div>
      ) : null}

      {error ? (
        <Callout tone="danger" title={error.message} className="mb-8">
          {error.remedy}
        </Callout>
      ) : null}

      <SettingGroup
        title="Model"
        description="Larger models hear better and run slower. The recommended one is the largest this machine handles comfortably."
      >
        {models.map((model) => (
          <ModelRow
            key={model.id}
            model={model}
            installed={isInstalled(model.id)}
            recommended={recommendation?.modelId === model.id}
            viable={recommendation?.viable.includes(model.id) ?? true}
            active={settings?.transcription.modelId === model.id}
            progress={downloads[model.id]}
            accelerated={recommendation?.accelerated ?? false}
            onDownload={() => void download(model.id)}
            onCancel={() => void ipc.cancelModelDownload()}
            onRemove={() => void remove(model.id)}
            onUse={() =>
              settings &&
              void update({
                transcription: { ...settings.transcription, modelId: model.id },
              })
            }
          />
        ))}
      </SettingGroup>

      <SettingGroup title="Options">
        <SettingRow
          label="Language"
          description="Whisper detects the language itself unless you pin one. Pinning helps when a recording switches between two."
          control={
            <Select
              value={settings?.transcription.language ?? "auto"}
              onValueChange={(language) =>
                settings && void update({ transcription: { ...settings.transcription, language } })
              }
              options={LANGUAGES}
            />
          }
        />

        <SettingRow
          label="Use the graphics card"
          description={
            machine?.acceleration === "available"
              ? "Much faster. Turn this off if a driver problem makes transcription fail."
              : "No usable GPU was found, so this has no effect until one is available."
          }
          control={
            <Switch
              checked={settings?.transcription.useGpu ?? true}
              disabled={machine?.acceleration !== "available"}
              onCheckedChange={(useGpu) =>
                settings && void update({ transcription: { ...settings.transcription, useGpu } })
              }
              aria-label="Use the graphics card for transcription"
            />
          }
        />

        <SettingRow
          label="Show text while recording"
          description={
            recommendation?.accelerated
              ? "Transcribes the last half-minute every few seconds so you can see it in the recording bar. It shares your graphics card with everything else, so expect other work to feel slower while recording."
              : "Transcribes the last half-minute every few seconds so you can see it in the recording bar. Without a GPU this runs on your processor and will make the rest of the machine stutter — worth trying before you rely on it in a meeting."
          }
          control={
            <Switch
              checked={settings?.transcription.liveTranscript ?? false}
              onCheckedChange={(liveTranscript) =>
                settings &&
                void update({ transcription: { ...settings.transcription, liveTranscript } })
              }
              aria-label="Show text while recording"
            />
          }
        />

        <SettingRow
          label="Tell voices apart"
          description="Guesses who is speaking in a recording made with one microphone, and labels the transcript. It gets two clearly different voices right and merges two similar ones rather than guessing — rename a speaker to correct it. Giving each person their own microphone in Audio sources is more reliable and takes precedence."
          control={
            <Switch
              checked={settings?.transcription.detectSpeakers ?? true}
              onCheckedChange={(detectSpeakers) =>
                settings &&
                void update({ transcription: { ...settings.transcription, detectSpeakers } })
              }
              aria-label="Tell voices apart in a single-microphone recording"
            />
          }
        />

        <SettingRow
          label="Timestamps in the summary"
          description="Gives the model the time each line was spoken. Worth it for meetings and interviews; wasted tokens on a short note."
          control={
            <Switch
              checked={settings?.transcription.timestamps ?? true}
              onCheckedChange={(timestamps) =>
                settings && void update({ transcription: { ...settings.transcription, timestamps } })
              }
              aria-label="Include timestamps in the summary prompt"
            />
          }
        />
      </SettingGroup>
    </Panel>
  );
}

function MachineReport({
  machine,
  recommendation,
}: {
  machine: Machine;
  recommendation: Recommendation | null;
}) {
  const gpu = machine.gpus.find((g) => g.kind === "discrete") ?? machine.gpus[0];

  const displayServer = useAppStore((state) => state.capabilities?.displayServer ?? null);

  return (
    <div className="space-y-3">
      {recommendation ? (
        <p className="text-sm leading-relaxed text-fg">{recommendation.headline}</p>
      ) : null}

      <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1.5 text-xs">
        <Fact icon={<Cpu />} label="Processor">
          {machine.cpuModel} · {machine.cpuThreads} threads
        </Fact>

        <Fact icon={<Zap />} label="Graphics">
          {gpu ? (
            <>
              {gpu.name} · {gpu.vramMb >= 1024 ? `${(gpu.vramMb / 1024).toFixed(0)} GB` : `${gpu.vramMb} MB`}
              {machine.acceleration === "available" ? ` · ${gpuBackend(displayServer)}` : " · not usable"}
            </>
          ) : (
            "none detected"
          )}
        </Fact>

        <Fact icon={<HardDrive />} label="Free space">
          {machine.freeDiskMb > 0
            ? `${bytes(machine.freeDiskMb * 1024 * 1024)} where models are stored`
            : "could not be measured"}
        </Fact>

        <Fact icon={<Cpu />} label="Memory">
          {bytes(machine.availableRamMb * 1024 * 1024)} available of{" "}
          {bytes(machine.totalRamMb * 1024 * 1024)}
        </Fact>
      </dl>
    </div>
  );
}

function Fact({
  icon,
  label,
  children,
}: {
  icon: React.ReactNode;
  label: string;
  children: React.ReactNode;
}) {
  return (
    <>
      <dt className="flex items-center gap-1.5 text-faint [&_svg]:size-3.5">
        {icon}
        {label}
      </dt>
      <dd data-numeric className="text-muted">
        {children}
      </dd>
    </>
  );
}

function NoteCallout({ note }: { note: CapabilityNote }) {
  return (
    <Callout tone={note.severity === "degraded" ? "warning" : "neutral"} title={note.title}>
      <p>{note.detail}</p>
      {note.remedy ? <p className="mt-1.5 text-fg">{note.remedy}</p> : null}
    </Callout>
  );
}

function ModelRow({
  model,
  installed,
  recommended,
  viable,
  active,
  progress,
  accelerated,
  onDownload,
  onCancel,
  onRemove,
  onUse,
}: {
  model: ModelSpec;
  installed: boolean;
  recommended: boolean;
  viable: boolean;
  active: boolean;
  progress: Progress | undefined;
  accelerated: boolean;
  onDownload: () => void;
  onCancel: () => void;
  onRemove: () => void;
  onUse: () => void;
}) {
  const speed = accelerated ? model.gpuSpeed : model.cpuSpeed;

  return (
    <ContextMenu
      content={
        <>
          <MenuLabel>{model.label}</MenuLabel>
          {progress ? (
            <MenuItem icon={<X />} onSelect={onCancel}>
              Cancel the download
            </MenuItem>
          ) : installed ? (
            <>
              <MenuItem icon={<Check />} disabled={active} onSelect={onUse}>
                Use for transcription
              </MenuItem>
              <MenuSeparator />
              <MenuItem icon={<Trash2 />} tone="danger" onSelect={onRemove}>
                Remove the download
              </MenuItem>
            </>
          ) : (
            <MenuItem icon={<Download />} onSelect={onDownload}>
              Install
            </MenuItem>
          )}
        </>
      }
    >
    <div
      className={cn(
        "flex items-center gap-3 px-3.5 py-3",
        active && "bg-accent-subtle",
      )}
    >
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium text-fg">{model.label}</span>


          {recommended ? <span className="text-2xs text-accent">Recommended</span> : null}
          {active ? <span className="text-2xs text-muted">Active</span> : null}
          {!viable ? <span className="text-2xs text-warning">Too large for this machine</span> : null}
        </div>

        <p className="mt-0.5 text-xs text-muted">{model.summary}</p>

        <p data-numeric className="mt-0.5 text-2xs text-faint">
          {bytes(model.bytes)} · about {speed}× real time {accelerated ? "on your GPU" : "on your CPU"}
        </p>

        {progress ? (
          <div className="mt-2 space-y-1">
            <div className="h-1 overflow-hidden rounded-full bg-inset">
              <div
                className="h-full rounded-full bg-accent transition-[width] duration-base ease-out"
                style={{ width: `${progress.percent}%` }}
              />
            </div>
            <p data-numeric className="text-2xs text-faint">
              {progress.verifying
                ? "Checking the download is intact…"
                : `${bytes(progress.downloadedBytes)} of ${bytes(progress.totalBytes)}`}
            </p>
          </div>
        ) : null}
      </div>

      <div className="flex shrink-0 items-center gap-1.5">
        {progress ? (
          <Button size="sm" variant="ghost" onClick={onCancel}>
            Cancel
          </Button>
        ) : installed ? (
          <>
            {!active ? (
              <Button size="sm" onClick={onUse}>
                Use
              </Button>
            ) : (
              <span className="inline-flex items-center gap-1.5 px-2 text-xs text-success [&_svg]:size-3.5">
                <Check />
                In use
              </span>
            )}
            <Button size="sm" variant="ghost" icon aria-label={`Remove ${model.label}`} onClick={onRemove}>
              <Trash2 />
            </Button>
          </>
        ) : (
          <Button size="sm" variant={recommended ? "primary" : "secondary"} onClick={onDownload}>
            <Download />
            Install
          </Button>
        )}
      </div>
    </div>
    </ContextMenu>
  );
}
