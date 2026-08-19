import { useCallback, useEffect, useState } from "react";
import { FolderInput, FolderOpen, NotebookPen, ShieldAlert, Trash2, X } from "lucide-react";
import { openPath } from "@tauri-apps/plugin-opener";
import { open as chooseFolder } from "@tauri-apps/plugin-dialog";

import { Button, Callout, Dialog, Panel, SettingGroup, SettingRow, Skeleton } from "@/components/ui";
import { bytes } from "@/lib/format";
import { ipc, type AppPaths } from "@/lib/ipc";
import { useAppStore } from "@/store/useAppStore";


export function StoragePanel() {
  const [paths, setPaths] = useState<AppPaths | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [deleted, setDeleted] = useState<number | null>(null);
  const [openError, setOpenError] = useState<string | null>(null);

  const recordings = useAppStore((state) => state.recordings);
  const refreshRecordings = useAppStore((state) => state.refreshRecordings);

  const refresh = useCallback(async () => {
    setPaths(await ipc.getPaths());
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);


  const open = async (path: string) => {
    setOpenError(null);
    try {
      await openPath(path);
    } catch (error) {
      setOpenError(
        `Could not open ${path}. ${error instanceof Error ? error.message : String(error)}`,
      );
    }
  };

  const deleteEverything = async () => {
    const count = await ipc.deleteAllRecordings();
    setDeleted(count);
    setConfirming(false);
    await Promise.all([refresh(), refreshRecordings()]);
  };

  return (
    <Panel title="Storage" description="Where your recordings are, and how to get rid of them.">
      <SettingGroup title="On this machine">
        <SettingRow
          label="Recordings"
          description={paths ? paths.recordingsDir : <Skeleton className="h-3 w-64" />}
          control={
            <div className="flex items-center gap-2">
              <span data-numeric className="text-xs text-muted">
                {paths ? bytes(paths.recordingsBytes) : "—"}
              </span>
              <Button
                size="sm"
                onClick={() => paths && void open(paths.recordingsDir)}
                aria-label="Open the recordings folder"
              >
                <FolderOpen />
                Open
              </Button>
            </div>
          }
        />

        <SettingRow
          label="Whisper models"
          description={paths ? paths.modelsDir : <Skeleton className="h-3 w-64" />}
          control={
            <div className="flex items-center gap-2">
              <span data-numeric className="text-xs text-muted">
                {paths ? bytes(paths.modelsBytes) : "—"}
              </span>
              <Button
                size="sm"
                onClick={() => paths && void open(paths.modelsDir)}
                aria-label="Open the models folder"
              >
                <FolderOpen />
                Open
              </Button>
            </div>
          }
        />

        <SettingRow
          label="Settings and database"
          description={paths ? paths.dataDir : <Skeleton className="h-3 w-64" />}
          control={
            <Button
              size="sm"
              onClick={() => paths && void open(paths.dataDir)}
              aria-label="Open the data folder"
            >
              <FolderOpen />
              Open
            </Button>
          }
        />
      </SettingGroup>

      <FolderGroup />

      <SettingGroup title="Delete everything">
        <SettingRow
          label={`${recordings.length} recording${recordings.length === 1 ? "" : "s"}`}
          description="Removes every transcript, every summary and every audio file. There is no undo and no copy anywhere else."
          control={
            <Button
              variant="danger"
              size="sm"
              disabled={recordings.length === 0}
              onClick={() => setConfirming(true)}
            >
              <Trash2 />
              Delete all
            </Button>
          }
        />
      </SettingGroup>

      {openError ? (
        <Callout tone="warning" title="Could not open the folder" className="mb-7">
          {openError}
        </Callout>
      ) : null}

      {deleted !== null ? (
        <Callout tone="neutral" className="mb-7">
          Deleted {deleted} recording{deleted === 1 ? "" : "s"} and their audio.
        </Callout>
      ) : null}

      <Dialog
        open={confirming}
        onOpenChange={setConfirming}
        title="Delete every recording?"
        description="This cannot be undone."
        footer={
          <>
            <Button variant="ghost" onClick={() => setConfirming(false)}>
              Cancel
            </Button>
            <Button variant="danger" onClick={() => void deleteEverything()}>
              Delete {recordings.length} recording{recordings.length === 1 ? "" : "s"}
            </Button>
          </>
        }
      >
        <Callout tone="danger" icon={<ShieldAlert />}>
          Every transcript, summary and audio file will be removed from this machine. ZScribe keeps
          no copy anywhere else, so there is nothing to restore from.
        </Callout>
      </Dialog>
    </Panel>
  );
}


function FolderGroup() {
  const settings = useAppStore((state) => state.settings);
  const update = useAppStore((state) => state.update);
  const [problem, setProblem] = useState<string | null>(null);

  if (!settings) return null;
  const folders = settings.folders;

  const pick = async (which: "watch" | "notes") => {
    setProblem(null);
    try {
      const chosen = await chooseFolder({ directory: true, multiple: false });
      if (typeof chosen !== "string") return;
      await update({ folders: { ...folders, [which]: chosen } });
    } catch (error) {
      setProblem(error instanceof Error ? error.message : String(error));
    }
  };

  const clear = (which: "watch" | "notes") =>
    void update({ folders: { ...folders, [which]: null } });

  return (
    <SettingGroup
      title="Folders"
      description="Optional, and off until you choose one."
    >
      <SettingRow
        label="Watch a folder"
        description={
          folders.watch
            ? `Anything droppable in ${folders.watch} is imported and transcribed, then moved into its “Imported” subfolder.`
            : "Point this at a folder and any audio or video dropped there is imported and transcribed on its own — a voice recorder's card, a phone's exports, a downloads folder. Files are moved into an “Imported” subfolder afterwards, never deleted."
        }
        control={
          <div className="flex items-center gap-1.5">
            {folders.watch ? (
              <Button size="sm" variant="ghost" icon onClick={() => clear("watch")} aria-label="Stop watching the folder">
                <X />
              </Button>
            ) : null}
            <Button size="sm" onClick={() => void pick("watch")}>
              <FolderInput />
              {folders.watch ? "Change" : "Choose"}
            </Button>
          </div>
        }
      />

      <SettingRow
        label="Write notes to a folder"
        description={
          folders.notes
            ? `Every finished recording is written to ${folders.notes} as Markdown, and rewritten there when it is renamed or summarised again.`
            : "Point this at an Obsidian vault — or any folder — and each finished recording is written there as a Markdown note, summary and transcript together. Renaming or summarising again updates the same file rather than adding another."
        }
        control={
          <div className="flex items-center gap-1.5">
            {folders.notes ? (
              <Button size="sm" variant="ghost" icon onClick={() => clear("notes")} aria-label="Stop writing notes">
                <X />
              </Button>
            ) : null}
            <Button size="sm" onClick={() => void pick("notes")}>
              <NotebookPen />
              {folders.notes ? "Change" : "Choose"}
            </Button>
          </div>
        }
      />

      {problem ? (
        <div className="px-4 py-3 text-xs text-warning">Could not choose a folder: {problem}</div>
      ) : null}
    </SettingGroup>
  );
}
