import { useEffect, useState } from "react";
import { CircleAlert, Keyboard } from "lucide-react";

import { Callout, Input, Panel, SettingGroup, SettingRow } from "@/components/ui";
import { ipc, toCommandError, type HotkeyStatus } from "@/lib/ipc";
import { useAppStore } from "@/store/useAppStore";

export function HotkeysPanel() {
  const settings = useAppStore((state) => state.settings);
  const capabilities = useAppStore((state) => state.capabilities);
  const update = useAppStore((state) => state.update);

  const [draft, setDraft] = useState("");
  const [preview, setPreview] = useState<string | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [status, setStatus] = useState<HotkeyStatus | null>(null);

  useEffect(() => {
    void ipc.getHotkeyStatus().then(setStatus);
  }, [settings?.hotkey]);

  useEffect(() => {
    if (settings) setDraft(settings.hotkey);
  }, [settings?.hotkey]);

  if (!settings) return null;

  const check = async (value: string) => {
    setDraft(value);
    try {
      setPreview(await ipc.validateHotkey(value));
      setProblem(null);
    } catch (raw) {
      setPreview(null);
      setProblem(toCommandError(raw).message);
    }
  };

  const commit = async () => {
    if (problem || !draft.trim() || draft === settings.hotkey) return;
    await update({ hotkey: draft });
    setStatus(await ipc.getHotkeyStatus());
  };

  return (
    <Panel
      title="Hotkeys"
      description="One key starts recording; the same key stops it."
    >
      <SettingGroup title="Record">
        <SettingRow
          label="Start and stop"
          description={
            preview
              ? `Will be shown as ${preview}`
              : "At least one modifier and one key — otherwise it would fire while you type."
          }
          stacked
          control={
            <Input
              value={draft}
              onChange={(event) => void check(event.target.value)}
              onBlur={() => void commit()}
              onKeyDown={(event) => {
                if (event.key === "Enter") void commit();
              }}
              placeholder="Ctrl+Alt+R"
              spellCheck={false}
              aria-label="Recording hotkey"
              aria-invalid={problem !== null}
            />
          }
        />
      </SettingGroup>

      {problem ? (
        <Callout tone="danger" icon={<CircleAlert />} className="mb-7">
          {problem}
        </Callout>
      ) : null}

      {status && !status.registered ? (
        <Callout tone="warning" icon={<Keyboard />} title="The hotkey is not active" className="mb-7">
          {status.problem}
          <span className="mt-1.5 block text-fg">
            You can still record from the Record button and from the tray menu.
          </span>
        </Callout>
      ) : null}

      {capabilities?.hotkey === "portal" ? (
        <Callout tone="neutral" icon={<Keyboard />} title="Your compositor owns this shortcut">
          On Wayland the compositor decides the final key combination, so it may differ from what
          you typed. ZScribe shows whichever one was actually bound.
        </Callout>
      ) : null}
    </Panel>
  );
}
