import { useEffect, useState } from "react";

import { Panel, SettingGroup, SettingRow, Switch } from "@/components/ui";
import { ipc } from "@/lib/ipc";
import { useAppStore } from "@/store/useAppStore";

export function SystemPanel() {
  const settings = useAppStore((state) => state.settings);
  const update = useAppStore((state) => state.update);
  const [autostart, setAutostart] = useState(false);

  useEffect(() => {
    void ipc.isAutostartEnabled().then(setAutostart);
  }, []);

  if (!settings) return null;
  const system = settings.system;
  const set = (patch: Partial<typeof system>) => void update({ system: { ...system, ...patch } });

  return (
    <Panel title="Startup & window" description="How ZScribe behaves on this computer.">
      <SettingGroup title="Startup">
        <SettingRow
          label="Start with the system"
          description="Useful when the hotkey is how you record — it only works while ZScribe is running."
          control={
            <Switch
              checked={autostart}
              onCheckedChange={(enabled) => {
                setAutostart(enabled);
                void ipc.setAutostart(enabled).catch(() => setAutostart(!enabled));
                set({ startWithOs: enabled });
              }}
              aria-label="Start with the system"
            />
          }
        />

        <SettingRow
          label="Start minimised"
          description="Goes straight to the tray instead of opening the window."
          control={
            <Switch
              checked={system.startMinimized}
              onCheckedChange={(startMinimized) => set({ startMinimized })}
              aria-label="Start minimised"
            />
          }
        />
      </SettingGroup>

      <SettingGroup title="Window">
        <SettingRow
          label="Close to the tray"
          description="Closing the window leaves ZScribe running. A recording in progress continues either way."
          control={
            <Switch
              checked={system.minimizeToTray}
              onCheckedChange={(minimizeToTray) => set({ minimizeToTray })}
              aria-label="Close to the tray"
            />
          }
        />

        <SettingRow
          label="Show notifications"
          description="Confirmations when something finishes. Failures are always shown regardless."
          control={
            <Switch
              checked={system.showNotifications}
              onCheckedChange={(showNotifications) => set({ showNotifications })}
              aria-label="Show notifications"
            />
          }
        />
      </SettingGroup>
    </Panel>
  );
}
