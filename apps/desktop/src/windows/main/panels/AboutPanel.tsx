import { useEffect, useState } from "react";
import { ExternalLink, FolderOpen, ShieldCheck } from "lucide-react";
import { openPath, openUrl } from "@tauri-apps/plugin-opener";

import { Button, Callout, Panel, SettingGroup, SettingRow, Skeleton } from "@/components/ui";
import { gpuBackend } from "@/lib/format";
import { ipc, type AppPaths, type Capabilities } from "@/lib/ipc";

export function AboutPanel() {
  const [version, setVersion] = useState("");
  const [gpu, setGpu] = useState<boolean | null>(null);
  const [paths, setPaths] = useState<AppPaths | null>(null);
  const [capabilities, setCapabilities] = useState<Capabilities | null>(null);

  useEffect(() => {
    void ipc.appVersion().then(setVersion);
    void ipc.gpuSupport().then(setGpu);
    void ipc.getPaths().then(setPaths);
    void ipc.getCapabilities().then(setCapabilities);
  }, []);

  return (
    <Panel title="About" description="What this is, and what it does with your data.">
      <Callout tone="neutral" icon={<ShieldCheck />} title="Nothing leaves this computer" className="mb-7">
        Audio is recorded here and transcribed here. There is no account, no server and no
        telemetry. The only time ZScribe uses the network is to download a Whisper model, and — if
        you choose a cloud provider in AI models — to send the finished transcript for summarising.
      </Callout>

      <SettingGroup title="Build">
        <SettingRow label="Version" description="ZScribe" control={<span data-numeric className="text-sm text-muted">{version}</span>} />
        <SettingRow
          label="GPU acceleration"
          description={
            gpu
              ? `This build includes the ${gpuBackend(capabilities?.displayServer)} backend, so a supported graphics card can be used.`
              : "This build has no GPU backend compiled in; transcription runs on the CPU."
          }
          control={<span className="text-sm text-muted">{gpu === null ? "—" : gpu ? gpuBackend(capabilities?.displayServer) : "CPU only"}</span>}
        />
        <SettingRow
          label="Session"
          description="How ZScribe registers its global hotkey here."
          control={
            <span className="text-sm text-muted">
              {capabilities ? `${capabilities.displayServer} · ${capabilities.hotkey}` : "—"}
            </span>
          }
        />
        <SettingRow
          label="Licence"
          description="GPL-3.0-or-later. The source is yours to read and change."
          control={
            <Button size="sm" onClick={() => void openUrl("https://www.gnu.org/licenses/gpl-3.0.html")}>
              <ExternalLink />
              Read it
            </Button>
          }
        />
      </SettingGroup>

      <SettingGroup title="Where things are">
        <PathRow label="Configuration" path={paths?.configDir} />
        <PathRow label="Data" path={paths?.dataDir} />


        <PathRow label="Logs" path={paths?.logsDir} />
      </SettingGroup>
    </Panel>
  );
}

function PathRow({ label, path }: { label: string; path: string | undefined }) {
  return (
    <SettingRow
      label={label}
      description={path ?? <Skeleton className="h-3 w-64" />}
      control={
        <Button size="sm" disabled={!path} onClick={() => path && void openPath(path)}>
          <FolderOpen />
          Open
        </Button>
      }
    />
  );
}
