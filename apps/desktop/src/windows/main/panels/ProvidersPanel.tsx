import { useCallback, useEffect, useState } from "react";
import { CircleAlert, CircleCheck, Check, Copy, Download, ExternalLink, Eye, EyeOff, Info, RefreshCw, ShieldCheck, X, Zap } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";

import {
  Button,
  Callout,
  ContextMenu,
  Input,
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
import {
  EVENTS,
  ipc,
  on,
  toCommandError,
  type CatalogueEntry,
  type ModelInfo,
  type PipelineFailure,
  type ProviderId,
  type PullProgress,
  type SecretBackend,
  type Suggestion,
} from "@/lib/ipc";
import { bytes } from "@/lib/format";
import { useAppStore } from "@/store/useAppStore";
import { Tags } from "../Tags";

const PROVIDERS: {
  id: ProviderId;
  label: string;
  needsKey: boolean;
  keyUrl?: string;
  docsUrl: string;
  note: string;
}[] = [
  {
    id: "ollama",
    label: "Ollama",
    needsKey: false,
    docsUrl: "https://ollama.com/library",
    note: "Runs entirely on your machine. No API key, no network, and your transcript never leaves the computer.",
  },
  {
    id: "gemini",
    label: "Google Gemini",
    needsKey: true,
    keyUrl: "https://aistudio.google.com/app/apikey",
    docsUrl: "https://ai.google.dev/gemini-api/docs/models",
    note: "A free tier is available and is enough for everyday use.",
  },
  {
    id: "openAiCompatible",
    label: "OpenAI-compatible",
    needsKey: true,
    keyUrl: "https://platform.openai.com/api-keys",
    docsUrl: "https://platform.openai.com/docs/models",
    note: "Works with OpenAI, OpenRouter, Groq, LM Studio and anything else speaking the same API — set the endpoint below.",
  },
];

const DEFAULT_ENDPOINTS: Record<ProviderId, string> = {
  ollama: "http://localhost:11434",
  gemini: "https://generativelanguage.googleapis.com/v1beta",
  openAiCompatible: "https://api.openai.com/v1",
};

export function ProvidersPanel() {
  const settings = useAppStore((state) => state.settings);
  const update = useAppStore((state) => state.update);

  const [backend, setBackend] = useState<SecretBackend | null>(null);
  const [models, setModels] = useState<ModelInfo[] | null>(null);
  const [modelsError, setModelsError] = useState<string | null>(null);
  const [loadingModels, setLoadingModels] = useState(false);
  const [hasKey, setHasKey] = useState(false);
  const [keyDraft, setKeyDraft] = useState("");
  const [keyVisible, setKeyVisible] = useState(false);
  const [keySaved, setKeySaved] = useState(false);

  const [catalogue, setCatalogue] = useState<CatalogueEntry[]>([]);
  const [suggestion, setSuggestion] = useState<Suggestion | null>(null);
  const [accelerated, setAccelerated] = useState<boolean | null>(null);
  const [pulls, setPulls] = useState<Record<string, PullProgress>>({});


  const [starting, setStarting] = useState<Record<string, boolean>>({});


  const [installError, setInstallError] = useState<string | null>(null);

  const active = settings?.activeProvider ?? "ollama";
  const meta = PROVIDERS.find((provider) => provider.id === active) ?? PROVIDERS[0]!;
  const profile = settings?.providers.find((p) => p.id === active);

  useEffect(() => {
    void ipc.getSecretBackend().then(setBackend);
    void ipc.listCatalogue().then(setCatalogue);
    void ipc.suggestLlm().then(setSuggestion);
    void ipc.llmAcceleration().then(setAccelerated);
  }, []);


  useEffect(() => {
    setModels(null);
    setModelsError(null);
    setKeyDraft("");
    setKeySaved(false);
    void ipc.hasApiKey(active).then(setHasKey);
  }, [active]);

  const loadModels = useCallback(async () => {
    setLoadingModels(true);
    setModelsError(null);
    try {
      setModels(await ipc.listModels(active));
    } catch (error) {
      const failure = toCommandError(error);
      setModelsError(`${failure.message} ${failure.remedy}`);
      setModels([]);
    } finally {
      setLoadingModels(false);
    }
  }, [active]);

  useEffect(() => {
    const unlisten = on<[string, PullProgress | null]>(EVENTS.llmProgress, ([id, progress]) => {
      setPulls((current) => {
        const next = { ...current };
        if (progress) next[id] = progress;
        else delete next[id];
        return next;
      });

      setStarting((current) => {
        if (!(id in current)) return current;
        const next = { ...current };
        delete next[id];
        return next;
      });


      if (!progress) {
        void loadModels();
        void ipc.llmAcceleration().then(setAccelerated);
      }
    });

    return () => {
      void unlisten.then((off) => off());
    };
  }, [loadModels]);


  useEffect(() => {
    const unlisten = on<PipelineFailure>(EVENTS.pipelineFailed, (event) => {
      if (event.stage !== "installing") return;
      setInstallError(`${event.error.message} ${event.error.remedy}`);
      setStarting({});
    });
    return () => {
      void unlisten.then((off) => off());
    };
  }, []);

  useEffect(() => {
    if (!meta.needsKey || hasKey) void loadModels();
  }, [meta.needsKey, hasKey, loadModels]);


  useEffect(() => {
    const refresh = () => {
      if (!meta.needsKey || hasKey) void loadModels();
    };
    window.addEventListener("focus", refresh);
    return () => window.removeEventListener("focus", refresh);
  }, [meta.needsKey, hasKey, loadModels]);

  if (!settings || !profile) return null;

  const setProfile = (patch: Partial<typeof profile>) =>
    void update({
      providers: settings.providers.map((p) => (p.id === active ? { ...p, ...patch } : p)),
    });

  const saveKey = async () => {
    await ipc.setApiKey(active, keyDraft);
    setHasKey(keyDraft.trim().length > 0);
    setKeyDraft("");
    setKeySaved(true);
    await loadModels();
  };

  const modelOptions = (models ?? []).map((model) => ({
    value: model.id,
    label: model.label,
  }));


  if (!modelOptions.some((option) => option.value === profile.model)) {
    modelOptions.unshift({ value: profile.model, label: profile.model });
  }

  return (
    <Panel
      title="AI models"
      description="Which model writes your summaries. Transcription is separate and always local."
    >
      <Callout tone="neutral" icon={<ShieldCheck />} className="mb-7">
        This choice only affects <strong className="text-fg">summaries</strong>. The recording and
        the transcript are produced on this machine either way — picking a cloud provider here
        sends it the finished transcript, never the audio.
      </Callout>

      <SettingGroup title="Provider">
        {PROVIDERS.map((provider) => (
          <button
            key={provider.id}
            onClick={() => void update({ activeProvider: provider.id })}
            aria-pressed={provider.id === active}
            className={cn(
              "block w-full px-4 py-3 text-left transition-colors duration-fast ease-out",
              provider.id === active ? "bg-accent-subtle" : "hover:bg-hover/50",
            )}
          >
            <span className="flex items-center gap-2">
              <span className="text-sm font-medium text-fg">{provider.label}</span>
              {provider.id === active ? <span className="text-2xs text-accent">Active</span> : null}
              {!provider.needsKey ? (
                <span className="text-2xs text-faint">no key needed</span>
              ) : null}
            </span>
            <p className="mt-1 text-xs leading-relaxed text-muted">{provider.note}</p>
          </button>
        ))}
      </SettingGroup>

      {meta.needsKey ? (
        <SettingGroup
          title="Authentication"
          description={
            backend === "keychain"
              ? "Keys are stored in your system keychain and never sent to this window."
              : "No system keychain was found, so keys are stored in an encrypted file in ZScribe's data directory."
          }
        >
          <SettingRow
            label="API key"
            description={
              hasKey ? "A key is stored. Enter a new one to replace it." : "No key stored yet."
            }
            stacked
            control={
              <div className="flex gap-2">
                <Input
                  type={keyVisible ? "text" : "password"}
                  value={keyDraft}
                  onChange={(event) => {
                    setKeyDraft(event.target.value);
                    setKeySaved(false);
                  }}
                  placeholder={hasKey ? "••••••••••••••••" : "Paste your API key"}
                  spellCheck={false}
                  autoComplete="off"
                  aria-label="API key"
                />
                <Button
                  variant="secondary"
                  icon
                  onClick={() => setKeyVisible((visible) => !visible)}
                  aria-label={keyVisible ? "Hide API key" : "Show API key"}
                >
                  {keyVisible ? <EyeOff /> : <Eye />}
                </Button>
                <Button
                  variant="primary"
                  disabled={keyDraft.trim().length === 0}
                  onClick={() => void saveKey()}
                >
                  Save
                </Button>
              </div>
            }
          />

          {meta.keyUrl ? (
            <SettingRow
              label="Don't have a key?"
              description="Opens the provider's console in your browser."
              control={
                <Button
                  variant="secondary"
                  onClick={() => meta.keyUrl && void openUrl(meta.keyUrl)}
                >
                  <ExternalLink />
                  Get a key
                </Button>
              }
            />
          ) : null}
        </SettingGroup>
      ) : null}

      {keySaved ? (
        <Callout tone="success" icon={<CircleCheck />} className="mb-7">
          Key saved.
        </Callout>
      ) : null}

      <SettingGroup title="Model">
        <SettingRow
          label="Model"
          description={
            active === "ollama"
              ? "Lists the models installed on this machine. Refreshes when you return to this window."
              : "Fetched live from the provider, so the list never goes stale."
          }
          control={
            <div className="flex w-64 items-center gap-2">
              {models === null && loadingModels ? (
                <Skeleton className="h-control-md flex-1" />
              ) : (
                <Select
                  value={profile.model}
                  onValueChange={(model) => setProfile({ model })}
                  options={modelOptions}
                  aria-label="Model"
                />
              )}
              <Button
                variant="secondary"
                icon
                onClick={() => void loadModels()}
                disabled={loadingModels}
                aria-label="Refresh the model list"
              >
                <RefreshCw className={cn(loadingModels && "animate-spin")} />
              </Button>
            </div>
          }
        />

        <SettingRow
          label="Documentation"
          description="What each model is good at, and what it costs."
          control={
            <Button variant="secondary" onClick={() => void openUrl(meta.docsUrl)}>
              <ExternalLink />
              Model docs
            </Button>
          }
        />

        <SettingRow
          label="Endpoint"
          description={
            active === "openAiCompatible"
              ? "Point this at OpenRouter, Groq, LM Studio or any other compatible server."
              : "Leave blank to use the provider's default."
          }
          stacked
          control={
            <Input
              value={profile.baseUrl ?? ""}
              onChange={(event) => setProfile({ baseUrl: event.target.value || null })}
              placeholder={DEFAULT_ENDPOINTS[active]}
              spellCheck={false}
              aria-label="Endpoint"
            />
          }
        />
      </SettingGroup>

      <PrivacyGroup local={active === "ollama"} />

      {active === "ollama" && (installError ?? modelsError) ? (
        <Callout
          tone="warning"
          icon={<CircleAlert />}
          title={installError ? "Could not install the model" : "Ollama isn’t reachable"}
          className="mb-7"
        >
          {installError ?? modelsError}
          <div className="mt-3 flex gap-2">
            <Button
              variant="secondary"
              size="sm"
              onClick={() => void openUrl("https://ollama.com/download")}
            >
              <ExternalLink />
              Get Ollama
            </Button>
            <Button
              variant="secondary"
              size="sm"
              onClick={() => {
                setInstallError(null);
                void loadModels();
              }}
            >
              <RefreshCw />
              Check again
            </Button>
          </div>
        </Callout>
      ) : null}

      {active === "ollama" ? (
        <SettingGroup
          title="Browse models"
          description="Installed through Ollama, in one step. The recommended one is the largest this machine runs comfortably."
        >
          {suggestion ? (
            <div className="border-b border-line-subtle p-3.5">
              <p className="text-sm leading-relaxed text-fg">{suggestion.headline}</p>
              {accelerated !== null ? (
                <p className="mt-1.5 inline-flex items-center gap-1.5 text-xs text-muted [&_svg]:size-3.5">
                  <Zap />
                  {accelerated
                    ? "Ollama is running the loaded model on your graphics card."
                    : "Ollama is running on the processor. It uses a graphics card automatically when a driver — ROCm for AMD, CUDA for NVIDIA — is installed; ZScribe cannot install one for you."}
                </p>
              ) : null}
            </div>
          ) : null}

          {catalogue.map((entry) => {
            const installed = (models ?? []).some((model) => model.id === entry.id);
            const progress = pulls[entry.id];
            const pending = starting[entry.id] ?? false;
            const busy = pending || progress !== undefined;


            const measured = progress !== undefined && progress.totalBytes > 0;
            const viable = suggestion?.viable.includes(entry.id) ?? true;

            const install = () => {
              setInstallError(null);
              setStarting((current) => ({ ...current, [entry.id]: true }));
              void ipc.installLlm(entry.id);
            };

            return (
              <ContextMenu
                key={entry.id}
                content={
                  <>
                    <MenuLabel>{entry.label}</MenuLabel>
                    {busy ? (
                      <MenuItem icon={<X />} onSelect={() => void ipc.cancelLlmInstall()}>
                        Cancel the download
                      </MenuItem>
                    ) : installed ? (
                      <MenuItem
                        icon={<Check />}
                        disabled={profile.model === entry.id}
                        onSelect={() => setProfile({ model: entry.id })}
                      >
                        Use for summaries
                      </MenuItem>
                    ) : (
                      <MenuItem icon={<Download />} onSelect={install}>
                        Install
                      </MenuItem>
                    )}
                    <MenuSeparator />
                    <MenuItem
                      icon={<Copy />}
                      onSelect={() => void navigator.clipboard.writeText(`ollama pull ${entry.id}`)}
                    >
                      Copy the pull command
                    </MenuItem>
                  </>
                }
              >
              <div className="flex items-center gap-3 px-3.5 py-3">
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="text-sm font-medium text-fg">{entry.label}</span>
                    {suggestion?.modelId === entry.id ? (
                      <span className="text-2xs text-accent">Recommended</span>
                    ) : null}
                    {profile.model === entry.id ? (
                      <span className="text-2xs text-muted">In use</span>
                    ) : null}
                    {!viable ? (
                      <span className="text-2xs text-warning">Too large for this machine</span>
                    ) : null}
                  </div>

                  <p className="mt-0.5 text-xs text-muted">{entry.summary}</p>
                  <p data-numeric className="mt-0.5 text-2xs text-faint">
                    {bytes(entry.megabytes * 1024 * 1024)} download
                  </p>

                  {busy ? (
                    <div className="mt-2 space-y-1">
                      <div className="h-1 overflow-hidden rounded-full bg-inset">
                        {measured ? (
                          <div
                            className="h-full rounded-full bg-accent transition-[width] duration-base ease-out"
                            style={{ width: `${progress.percent}%` }}
                          />
                        ) : (


                          <div className="h-full w-1/3 rounded-full bg-accent motion-safe:animate-indeterminate" />
                        )}
                      </div>
                      <p data-numeric className="text-2xs text-faint">
                        {measured
                          ? `${bytes(progress.downloadedBytes)} of ${bytes(progress.totalBytes)}`
                          : (progress?.stage ?? "Starting…")}
                      </p>
                    </div>
                  ) : null}
                </div>

                <div className="flex shrink-0 items-center gap-1.5">
                  {busy ? (
                    <Button size="sm" variant="ghost" onClick={() => void ipc.cancelLlmInstall()}>
                      Cancel
                    </Button>
                  ) : installed ? (
                    profile.model === entry.id ? (
                      <span className="inline-flex items-center gap-1.5 px-2 text-xs text-success [&_svg]:size-3.5">
                        <Check />
                        In use
                      </span>
                    ) : (
                      <Button size="sm" onClick={() => setProfile({ model: entry.id })}>
                        Use
                      </Button>
                    )
                  ) : (
                    <Button
                      size="sm"
                      variant={suggestion?.modelId === entry.id ? "primary" : "secondary"}
                      onClick={install}
                    >
                      <Download />
                      Install
                    </Button>
                  )}
                </div>
              </div>
              </ContextMenu>
            );
          })}
        </SettingGroup>
      ) : null}

      {active === "ollama" ? (
        <Callout tone="neutral" icon={<Info />} title="Adding another model">
          Anything not in the list above can be pulled from a terminal; return to this window and
          the list refreshes on its own:
          <code
            data-selectable
            className="mt-2 block rounded-md bg-inset px-2.5 py-1.5 font-mono text-xs text-fg"
          >
            ollama pull qwen2.5:7b
          </code>
          <span className="mt-2 block">
            A 7B model writes usable summaries. For long or rambling recordings, 12–14B is the
            point where they become reliable.
          </span>
        </Callout>
      ) : null}

      {modelsError && active !== "ollama" ? (
        <Callout tone="warning" icon={<CircleAlert />} title="Could not load the model list">
          {modelsError}
        </Callout>
      ) : null}
    </Panel>
  );
}


function PrivacyGroup({ local }: { local: boolean }) {
  const settings = useAppStore((state) => state.settings);
  const update = useAppStore((state) => state.update);
  if (!settings) return null;

  const privacy = settings.privacy;
  const set = (patch: Partial<typeof privacy>) =>
    void update({ privacy: { ...privacy, ...patch } });

  return (
    <SettingGroup
      title="Before it is sent"
      description={
        local
          ? "Ollama runs on this machine, so nothing is sent and nothing is removed. These apply the moment you switch to a provider that isn't local."
          : "Your transcript is about to reach a company's servers. These stay behind."
      }
    >
      <SettingRow
        label="Remove contact details"
        description="Email addresses, phone numbers, card and account numbers, IBANs. A summary loses nothing without them."
        control={
          <Switch
            checked={privacy.redactContacts}
            onCheckedChange={(redactContacts) => set({ redactContacts })}
            aria-label="Remove contact details"
          />
        }
      />

      <SettingRow
        label="Replace people's names"
        description="Every speaker this recording has a name for becomes “[name 1]”, in the transcript and the headings alike. Costs something real: action items come back assigned to a placeholder rather than to Anna."
        control={
          <Switch
            checked={privacy.redactSpeakers}
            onCheckedChange={(redactSpeakers) => set({ redactSpeakers })}
            aria-label="Replace people's names"
          />
        }
      />

      <SettingRow
        label="Never send these words"
        description="A client, an employer, a project — anything that should not appear in someone else's logs. Matched as whole words, ignoring case."
        stacked
        control={
          <Tags
            tags={privacy.redactTerms}
            noun="word"
            placeholder="Acme, Project Atlas…"
            onChange={async (redactTerms) => {
              await update({ privacy: { ...privacy, redactTerms } });
            }}
          />
        }
      />

      <div className="px-4 py-3">
        <p className="inline-flex items-start gap-1.5 text-xs leading-relaxed text-faint [&_svg]:mt-0.5 [&_svg]:size-3.5 [&_svg]:shrink-0">
          <Info />
          <span>
            This is not anonymisation. Patterns are found reliably; a name is only removed if
            ZScribe knows it — from the recording's speakers or from the list above. Everything
            you see in ZScribe is always the unredacted original.
          </span>
        </p>
      </div>
    </SettingGroup>
  );
}
