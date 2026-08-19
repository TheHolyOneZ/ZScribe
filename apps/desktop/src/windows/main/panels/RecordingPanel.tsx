import { useEffect, useState } from "react";
import { History, Info } from "lucide-react";

import {
  Callout,
  Panel,
  Select,
  SettingGroup,
  SettingRow,
  Switch,
} from "@/components/ui";
import { ipc, type InputDevice, type SourceAvailability, type SystemAudioDevice } from "@/lib/ipc";
import { SourceList } from "./SourceList";
import { useAppStore } from "@/store/useAppStore";


function label(seconds: number): string {
  if (seconds < 60) return `${seconds} seconds`;
  const minutes = seconds / 60;
  return minutes === 1 ? "minute" : `${minutes} minutes`;
}

export function RecordingPanel() {
  const settings = useAppStore((state) => state.settings);
  const update = useAppStore((state) => state.update);


  const [devices, setDevices] = useState<InputDevice[] | null>(null);
  const [systemAudio, setSystemAudio] = useState<SystemAudioDevice[]>([]);
  const [availability, setAvailability] = useState<SourceAvailability[]>([]);

  useEffect(() => {
    void ipc.listInputDevices().then(setDevices);
    void ipc.listSystemAudio().then(setSystemAudio);
    void ipc.checkSources().then(setAvailability);
  }, []);

  if (!settings) return null;
  const recording = settings.recording;
  const set = (patch: Partial<typeof recording>) =>
    void update({ recording: { ...recording, ...patch } });


  return (
    <Panel
      title="Audio sources"
      description="What gets captured, and how the people in the room find out."
    >
      <SettingGroup
        title="Sources"
        description="What gets recorded. Name each one and the transcript says who said what; leave the list empty and ZScribe records the system's default microphone without attributing anything."
      >
        <div className="p-3.5">
          <SourceList
            sources={recording.sources}
            microphones={devices ?? []}
            systemAudio={systemAudio}
            availability={availability}
            onChange={(sources) => {
              set({ sources });

              void ipc.checkSources().then(setAvailability);
            }}
          />
        </div>
      </SettingGroup>

      <SettingGroup
        title="Letting people know"
        description="Neither of these is required, and neither is enough on its own — but both make it harder to record someone by accident."
      >
        <SettingRow
          label="Play a tone when recording starts"
          description="A short sound, audible in the room."
          control={
            <Switch
              checked={recording.announceTone}
              onCheckedChange={(announceTone) => set({ announceTone })}
              aria-label="Play a tone when recording starts"
            />
          }
        />

        <SettingRow
          label="Note consent in the transcript"
          description="Adds a line at the top of the transcript recording that everyone present agreed."
          control={
            <Switch
              checked={recording.consentNote}
              onCheckedChange={(consentNote) => set({ consentNote })}
              aria-label="Note consent in the transcript"
            />
          }
        />
      </SettingGroup>

      <SettingGroup
        title="Before you press record"
        description="The one thing a recorder in the cloud cannot do."
      >
        <SettingRow
          label="Include what was already being said"
          description={
            recording.rewindSeconds > 0
              ? `Pressing record captures the previous ${label(recording.rewindSeconds)} as well. The microphone is open the whole time ZScribe is running — nothing is written to disk, and the buffer is discarded when the app closes.`
              : "Keeps the last few minutes of audio in memory so that starting a recording captures what was said before you remembered to. The cost: the microphone is open the whole time ZScribe is running."
          }
          control={
            <Select
              value={String(recording.rewindSeconds)}
              onValueChange={(value) => set({ rewindSeconds: Number(value) })}
              options={[
                { value: "0", label: "Off" },
                { value: "30", label: "30 seconds" },
                { value: "60", label: "1 minute" },
                { value: "120", label: "2 minutes" },
                { value: "300", label: "5 minutes" },
              ]}
              aria-label="Include what was already being said"
            />
          }
        />

        {recording.rewindSeconds > 0 ? (
          <div className="px-4 py-3">
            <p className="inline-flex items-start gap-1.5 text-xs leading-relaxed text-muted [&_svg]:mt-0.5 [&_svg]:size-3.5 [&_svg]:shrink-0">
              <History />
              <span>
                Held in memory only — there is no file and no temporary file, and the buffer is
                overwritten continuously. It is not used when you have named several audio sources:
                the buffer holds one microphone, and putting it in front of one track would shift
                that person against the others.
              </span>
            </p>
          </div>
        ) : null}
      </SettingGroup>

      <SettingGroup title="Afterwards">
        <SettingRow
          label="Keep the audio"
          description="Off means the recording is deleted once its transcript exists. The text stays; you just cannot transcribe it again."
          control={
            <Switch
              checked={recording.keepAudio}
              onCheckedChange={(keepAudio) => set({ keepAudio })}
              aria-label="Keep the audio after transcription"
            />
          }
        />
      </SettingGroup>

      <Callout tone="neutral" icon={<Info />}>
        Recording continues when you close this window. The tray icon says so while it does, and
        that indicator cannot be turned off.
      </Callout>
    </Panel>
  );
}
