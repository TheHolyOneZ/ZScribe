import { useState } from "react";
import { Check, Copy, Pencil, Plus, Trash2 } from "lucide-react";

import {
  Button,
  Callout,
  ContextMenu,
  Dialog,
  Input,
  MenuItem,
  MenuLabel,
  MenuSeparator,
  Panel,
  Select,
  SettingGroup,
  SettingRow,
  Textarea,
} from "@/components/ui";
import { cn } from "@/lib/cn";
import { ipc, type Template } from "@/lib/ipc";
import { useAppStore } from "@/store/useAppStore";


const SUMMARY_LANGUAGES: { value: string; label: string }[] = [
  { value: "auto", label: "Match the recording" },
  { value: "English", label: "English" },
  { value: "German", label: "German" },
  { value: "French", label: "French" },
  { value: "Spanish", label: "Spanish" },
  { value: "Italian", label: "Italian" },
  { value: "Portuguese", label: "Portuguese" },
  { value: "Dutch", label: "Dutch" },
  { value: "Polish", label: "Polish" },
  { value: "Russian", label: "Russian" },
  { value: "Ukrainian", label: "Ukrainian" },
  { value: "Turkish", label: "Turkish" },
  { value: "Arabic", label: "Arabic" },
  { value: "Hindi", label: "Hindi" },
  { value: "Japanese", label: "Japanese" },
  { value: "Korean", label: "Korean" },
  { value: "Chinese", label: "Chinese" },
];


export function TemplatesPanel() {
  const settings = useAppStore((state) => state.settings);
  const templates = useAppStore((state) => state.templates);
  const update = useAppStore((state) => state.update);
  const hydrate = useAppStore((state) => state.hydrate);

  const [editing, setEditing] = useState<Template | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<Template | null>(null);

  if (!settings) return null;

  const active = settings.templateId;
  const custom = settings.customTemplates;
  const isCustom = (id: string) => custom.some((t) => t.id === id);

  const save = async (template: Template) => {
    const existing = custom.some((t) => t.id === template.id);
    await update({
      customTemplates: existing
        ? custom.map((t) => (t.id === template.id ? template : t))
        : [...custom, template],
    });
    setEditing(null);


    await hydrate();
  };

  const remove = async (template: Template) => {
    setConfirmDelete(null);
    await update({
      customTemplates: custom.filter((t) => t.id !== template.id),


      ...(active === template.id ? { templateId: "self-note" } : {}),
    });
    await hydrate();
  };

  const duplicate = async (from: Template) => {
    const copy = await ipc.deriveTemplate(from.id, `${from.name} (copy)`);
    setEditing(copy);
  };

  return (
    <Panel
      title="Summary styles"
      description="What the summary should look for. Chosen when the recording starts; you can summarise again with a different one afterwards."
      actions={
        <Button
          size="sm"
          variant="primary"
          onClick={() => void duplicate(templates.find((t) => t.id === active) ?? templates[0]!)}
        >
          <Plus />
          New template
        </Button>
      }
    >
      <SettingGroup title="Template">
        {templates.map((template) => (
          <ContextMenu
            key={template.id}
            content={
              <>
                <MenuLabel>{template.name}</MenuLabel>
                <MenuItem
                  icon={<Check />}
                  disabled={template.id === active}
                  onSelect={() => void update({ templateId: template.id })}
                >
                  Use this style
                </MenuItem>
                <MenuSeparator />
                <MenuItem icon={<Copy />} onSelect={() => void duplicate(template)}>
                  Duplicate
                </MenuItem>


                <MenuItem
                  icon={<Pencil />}
                  disabled={!isCustom(template.id)}
                  onSelect={() => setEditing(template)}
                >
                  Edit
                </MenuItem>
                <MenuSeparator />
                <MenuItem
                  icon={<Trash2 />}
                  tone="danger"
                  disabled={!isCustom(template.id)}
                  onSelect={() => setConfirmDelete(template)}
                >
                  Delete…
                </MenuItem>
              </>
            }
          >
          <div
            className={cn(
              "flex items-start gap-3 px-4 py-3 transition-colors duration-fast ease-out",
              template.id === active ? "bg-accent-subtle" : "hover:bg-hover/50",
            )}
          >
            <button
              onClick={() => void update({ templateId: template.id })}
              aria-pressed={template.id === active}
              className="min-w-0 flex-1 text-left"
            >
              <span className="flex items-center gap-2">
                <span className="text-sm font-medium text-fg">{template.name}</span>
                {template.id === active ? (
                  <span className="text-2xs text-accent">Active</span>
                ) : null}
                {isCustom(template.id) ? (
                  <span className="text-2xs text-faint">yours</span>
                ) : null}
              </span>
              <p className="mt-1 text-xs leading-relaxed text-muted">
                {template.description || "No description."}
              </p>
            </button>

            <div className="flex shrink-0 items-center gap-0.5">
              <Button
                size="sm"
                variant="ghost"
                icon
                aria-label={`Duplicate ${template.name}`}
                onClick={() => void duplicate(template)}
              >
                <Copy />
              </Button>

              {isCustom(template.id) ? (
                <>
                  <Button
                    size="sm"
                    variant="ghost"
                    icon
                    aria-label={`Edit ${template.name}`}
                    onClick={() => setEditing(template)}
                  >
                    <Pencil />
                  </Button>
                  <Button
                    size="sm"
                    variant="ghost"
                    icon
                    aria-label={`Delete ${template.name}`}
                    onClick={() => setConfirmDelete(template)}
                  >
                    <Trash2 />
                  </Button>
                </>
              ) : null}
            </div>
          </div>
          </ContextMenu>
        ))}
      </SettingGroup>

      <SettingGroup
        title="Language"
        description="Transcription always stays in the language that was spoken. This only changes the language the summary is written in."
      >
        <SettingRow
          label="Summary language"
          description={
            settings.summaryLanguage === "auto" || !settings.summaryLanguage
              ? "Written in whatever language the recording is in."
              : `Translated into ${settings.summaryLanguage}, whatever was spoken.`
          }
          control={
            <Select
              value={
                SUMMARY_LANGUAGES.some((l) => l.value === settings.summaryLanguage)
                  ? settings.summaryLanguage
                  : "auto"
              }
              onValueChange={(summaryLanguage) => void update({ summaryLanguage })}
              options={SUMMARY_LANGUAGES}
              aria-label="Summary language"
            />
          }
        />
      </SettingGroup>

      <SettingGroup
        title="Instructions"
        description="What the active template asks the model to produce. Built-in templates are read-only — duplicate one to change it."
      >
        <div className="p-3.5">
          <pre
            data-selectable
            className="max-h-72 overflow-auto font-mono text-xs leading-relaxed whitespace-pre-wrap text-muted"
          >
            {templates.find((t) => t.id === active)?.instructions ?? ""}
          </pre>
        </div>
      </SettingGroup>

      <Editor template={editing} onCancel={() => setEditing(null)} onSave={save} />

      <Dialog
        open={confirmDelete !== null}
        onOpenChange={(open) => !open && setConfirmDelete(null)}
        title={`Delete “${confirmDelete?.name}”?`}
        description="Summaries already written with it are unaffected."
        footer={
          <>
            <Button variant="ghost" onClick={() => setConfirmDelete(null)}>
              Cancel
            </Button>
            <Button
              variant="danger"
              onClick={() => confirmDelete && void remove(confirmDelete)}
            >
              Delete
            </Button>
          </>
        }
      >
        <p className="text-sm text-muted">This template will be removed. There is no undo.</p>
      </Dialog>
    </Panel>
  );
}

function Editor({
  template,
  onCancel,
  onSave,
}: {
  template: Template | null;
  onCancel: () => void;
  onSave: (template: Template) => void;
}) {
  const [draft, setDraft] = useState<Template | null>(null);
  const current = draft ?? template;


  if (template && draft && draft.id !== template.id) setDraft(template);

  const set = (patch: Partial<Template>) => current && setDraft({ ...current, ...patch });

  return (
    <Dialog
      open={template !== null}
      onOpenChange={(open) => {
        if (!open) {
          setDraft(null);
          onCancel();
        }
      }}
      title="Edit template"
      description="Describe the sections you want. The rules against inventing content are added automatically."
      width="lg"
      footer={
        <>
          <Button
            variant="ghost"
            onClick={() => {
              setDraft(null);
              onCancel();
            }}
          >
            Cancel
          </Button>
          <Button
            variant="primary"
            disabled={!current?.name.trim() || !current?.instructions.trim()}
            onClick={() => {
              if (!current) return;
              onSave({ ...current, name: current.name.trim() });
              setDraft(null);
            }}
          >
            Save
          </Button>
        </>
      }
    >
      {current ? (
        <div className="space-y-4">
          <Field label="Name">
            <Input
              value={current.name}
              onChange={(event) => set({ name: event.target.value })}
              placeholder="Board meeting"
              aria-label="Template name"
            />
          </Field>

          <Field label="Description" hint="One line, shown under the name in this list.">
            <Input
              value={current.description}
              onChange={(event) => set({ description: event.target.value })}
              placeholder="Decisions and votes, with who proposed what."
              aria-label="Template description"
            />
          </Field>

          <Field
            label="Instructions"
            hint="Ask for the sections you want, by name. Markdown headings work well because the summary is rendered as Markdown."
          >
            <Textarea
              value={current.instructions}
              onChange={(event) => set({ instructions: event.target.value })}
              rows={14}
              spellCheck={false}
              className="font-mono text-xs"
              aria-label="Template instructions"
            />
          </Field>

          <Callout tone="neutral">
            You do not need to tell it to avoid inventing things, to answer in the recording's own
            language, or to reply in Markdown — every template gets those rules already.
          </Callout>
        </div>
      ) : null}
    </Dialog>
  );
}

function Field({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="space-y-1.5">
      <p className="text-xs font-medium text-fg">{label}</p>
      {hint ? <p className="text-2xs leading-relaxed text-faint">{hint}</p> : null}
      {children}
    </div>
  );
}
