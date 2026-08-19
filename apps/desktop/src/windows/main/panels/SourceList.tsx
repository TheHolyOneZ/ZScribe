import { Circle, CircleCheck, Mic, MonitorSpeaker, Pencil, Plus, Trash2 } from "lucide-react";

import {
  Button,
  Callout,
  ContextMenu,
  Input,
  MenuItem,
  MenuLabel,
  MenuSeparator,
  Select,
  Switch,
} from "@/components/ui";
import { cn } from "@/lib/cn";
import type { InputDevice, SourceAvailability, SourceProfile, SystemAudioDevice } from "@/lib/ipc";
import { useAppStore } from "@/store/useAppStore";


export function SourceList({
  sources,
  microphones,
  systemAudio,
  availability,
  onChange,
}: {
  sources: SourceProfile[];
  microphones: InputDevice[];
  systemAudio: SystemAudioDevice[];
  availability: SourceAvailability[];
  onChange: (next: SourceProfile[]) => void;
}) {
  const update = (index: number, patch: Partial<SourceProfile>) =>
    onChange(sources.map((source, i) => (i === index ? { ...source, ...patch } : source)));

  const remove = (index: number) => onChange(sources.filter((_, i) => i !== index));


  const focusName = (index: number) =>
    requestAnimationFrame(() => {
      const field = document.querySelector<HTMLInputElement>(`[data-source-name="${index}"]`);
      field?.focus();
      field?.select();
    });

  const add = (kind: SourceProfile["kind"]) => {
    const taken = new Set(sources.map((s) => s.deviceId));
    const pool = kind === "systemAudio" ? systemAudio : microphones;
    const free = pool.find((device) => !taken.has(device.id)) ?? pool[0];
    if (!free) return;

    onChange([
      ...sources,
      { deviceId: free.id, name: "", kind, enabled: true },
    ]);
  };

  const noSystemAudio = systemAudio.length === 0;


  const displayServer = useAppStore((state) => state.capabilities?.displayServer);
  const onLinux = displayServer === "x11" || displayServer === "wayland";


  const duplicated = (() => {
    const rows = new Map<string, string[]>();
    for (const source of sources.filter((s) => s.enabled)) {
      const names = rows.get(source.deviceId) ?? [];
      rows.set(source.deviceId, [...names, source.name || "an unnamed source"]);
    }
    return [...rows.values()].filter((names) => names.length > 1);
  })();

  return (
    <div className="space-y-3">
      {sources.length === 0 ? (
        <p className="text-xs leading-relaxed text-muted">
          Nothing added, so ZScribe records the single microphone above and the transcript does not
          say who spoke. Add a source per person to change that.
        </p>
      ) : (
        <div className="divide-y divide-line-subtle overflow-hidden rounded-lg border border-line-subtle bg-surface">
          {sources.map((source, index) => {
            const devices = source.kind === "systemAudio" ? systemAudio : microphones;
            const known = devices.some((device) => device.id === source.deviceId);


            const checked = availability.find((entry) => entry.deviceId === source.deviceId);
            const here = checked ? checked.available : known;

            return (
              <ContextMenu
                key={index}
                content={
                  <>
                    <MenuLabel>{source.name || "Unnamed source"}</MenuLabel>
                    <MenuItem icon={<Pencil />} onSelect={() => focusName(index)}>
                      Rename
                    </MenuItem>
                    <MenuItem
                      icon={source.enabled ? <Circle /> : <CircleCheck />}
                      onSelect={() => update(index, { enabled: !source.enabled })}
                    >
                      {source.enabled ? "Do not record this" : "Record this"}
                    </MenuItem>
                    <MenuSeparator />
                    <MenuItem icon={<Trash2 />} tone="danger" onSelect={() => remove(index)}>
                      Remove
                    </MenuItem>
                  </>
                }
              >
              <div className="space-y-2 px-3.5 py-3">
                <div className="flex items-center gap-2">
                  <span
                    aria-hidden
                    className={cn(
                      "shrink-0 [&_svg]:size-3.5",
                      source.enabled ? "text-muted" : "text-faint",
                    )}
                  >
                    {source.kind === "systemAudio" ? <MonitorSpeaker /> : <Mic />}
                  </span>

                  <Input
                    data-source-name={index}
                    value={source.name}
                    onChange={(event) => update(index, { name: event.target.value })}
                    placeholder={source.kind === "systemAudio" ? "Call, video…" : "Max Kruger"}
                    aria-label="Name for this source"
                    className="flex-1"
                  />


                  {!here ? (
                    <span className="shrink-0 text-2xs text-warning">not connected</span>
                  ) : null}

                  <Switch
                    checked={source.enabled}
                    onCheckedChange={(enabled) => update(index, { enabled })}
                    aria-label={`Record ${source.name || "this source"}`}
                  />

                  <Button
                    size="sm"
                    variant="ghost"
                    icon
                    aria-label="Remove this source"
                    onClick={() => remove(index)}
                  >
                    <Trash2 />
                  </Button>
                </div>

                <Select
                  value={source.deviceId}
                  onValueChange={(deviceId) => update(index, { deviceId })}
                  options={[
                    ...devices.map((device) => ({ value: device.id, label: device.name })),


                    ...(known
                      ? []
                      : [{ value: source.deviceId, label: `${source.deviceId} (not connected)` }]),
                  ]}
                  aria-label="Device for this source"
                />
              </div>
              </ContextMenu>
            );
          })}
        </div>
      )}

      <div className="flex items-center gap-2">
        <Button size="sm" onClick={() => add("microphone")} disabled={microphones.length === 0}>
          <Plus />
          Microphone
        </Button>
        <Button size="sm" onClick={() => add("systemAudio")} disabled={noSystemAudio}>
          <Plus />
          System audio
        </Button>
      </div>

      {microphones.length === 0 ? (
        <Callout tone="neutral" title="No microphone is available">
          ZScribe offers the same devices your system does — if one is listed in the operating
          system's sound settings, it appears here.
          {onLinux ? null : (
            <span className="mt-1.5 block">
              A remote-desktop session is the usual cause on Windows: it replaces the machine's
              sound devices with a single redirected pair for every application, so a microphone
              plugged into the machine itself is not offered. Recording needs the machine's own
              screen. Hardware listed in Device Manager is not the same thing — that shows drivers,
              not devices an application can open.
            </span>
          )}
        </Callout>
      ) : null}

      {noSystemAudio ? (
        <Callout tone="neutral">
          {onLinux ? (
            <>
              No capturable output was found. On Linux this needs PulseAudio or PipeWire, which
              provide a monitor for each output; ZScribe reads the list with <code>pactl</code>.
            </>
          ) : (
            <>
              No capturable output was found. Every output your system knows about can normally be
              recorded, so this usually means none are connected.
            </>
          )}
        </Callout>
      ) : null}

      {sources.some((source) => {
        const checked = availability.find((entry) => entry.deviceId === source.deviceId);
        return checked ? !checked.available : false;
      }) ? (
        <Callout tone="warning" title="Some devices are not connected">
          Their settings are kept, so plugging them back in is all it takes. Recording continues
          with whatever is here.
        </Callout>
      ) : null}

      {duplicated.length > 0 ? (
        <Callout tone="warning" title="The same device is selected twice">
          {duplicated.map((names) => names.join(" and ")).join("; ")} point at one device, so they
          would record the same audio and nothing could tell them apart. Give each source its own
          device.
        </Callout>
      ) : null}

      {sources.filter((s) => s.enabled).length > 1 ? (
        <Callout tone="neutral" title="How attribution works">
          Every source is transcribed separately, and each line is credited to whichever one heard
          it loudest — so the person's own microphone wins over everyone else's.
          <span className="mt-1.5 block">
            It works well with headsets and lapel microphones. Two laptops on the same table hear
            each other almost equally, and there it will guess.
          </span>
        </Callout>
      ) : null}
    </div>
  );
}
