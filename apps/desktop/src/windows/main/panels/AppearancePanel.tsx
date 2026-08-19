import { Panel, Select, SettingGroup, SettingRow, Slider } from "@/components/ui";
import { useAppStore } from "@/store/useAppStore";

export function AppearancePanel() {
  const settings = useAppStore((state) => state.settings);
  const update = useAppStore((state) => state.update);

  if (!settings) return null;
  const appearance = settings.appearance;

  return (
    <Panel title="Appearance" description="How ZScribe looks.">
      <SettingGroup title="Theme">
        <SettingRow
          label="Theme"
          description="Both themes are fully defined; “System” follows your desktop."
          control={
            <Select
              value={appearance.theme}
              onValueChange={(theme) =>
                void update({ appearance: { ...appearance, theme: theme as typeof appearance.theme } })
              }
              options={[
                { value: "system", label: "Follow the system" },
                { value: "dark", label: "Dark" },
                { value: "light", label: "Light" },
              ]}
              aria-label="Theme"
              className="w-48"
            />
          }
        />

        <SettingRow
          label="Opacity"
          description="Below 40% the window would be unusable, so that is the floor."
          control={
            <div className="flex w-48 items-center gap-3">
              <Slider
                min={40}
                max={100}
                step={5}
                value={appearance.opacity}
                onValueChange={(opacity) => void update({ appearance: { ...appearance, opacity } })}
                aria-label="Window opacity"
              />
              <span data-numeric className="w-9 shrink-0 text-right text-xs text-muted">
                {appearance.opacity}%
              </span>
            </div>
          }
        />
      </SettingGroup>

      <SettingGroup
        title="Recording bar"
        description="The small bar that sits on top of everything while you record."
      >
        <SettingRow
          label="Opacity"
          description="See-through keeps it out of the way of what you are reading. Solid is easier to read itself."
          control={
            <div className="flex w-48 items-center gap-3">
              <Slider
                min={40}
                max={100}
                step={4}
                value={appearance.recorderOpacity}
                onValueChange={(recorderOpacity) =>
                  void update({ appearance: { ...appearance, recorderOpacity } })
                }
                aria-label="Recording bar opacity"
              />
              <span data-numeric className="w-9 shrink-0 text-right text-xs text-muted">
                {appearance.recorderOpacity}%
              </span>
            </div>
          }
        />
      </SettingGroup>
    </Panel>
  );
}
