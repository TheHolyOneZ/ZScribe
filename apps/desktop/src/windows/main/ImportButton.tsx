import { useEffect, useRef, useState, type ReactNode } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import {
  ChevronDown,
  ChevronRight,
  CircleAlert,
  CircleCheck,
  FileAudio,
  Import,
  Link2,
  Loader2,
} from "lucide-react";

import { Button, Callout, Dialog, Input } from "@/components/ui";
import { Copyable } from "@/components/Copyable";
import { cn } from "@/lib/cn";
import { useEta } from "@/lib/eta";
import {
  EVENTS,
  ipc,
  on,
  toCommandError,
  type CommandError,
  type LinkSupport,
  type Platform,
  type StageProgress,
} from "@/lib/ipc";
import { useAppStore } from "@/store/useAppStore";


const PLATFORM_NAMES: Record<Platform, string> = {
  windows: "Windows",
  macos: "macOS",
  linux: "Linux",
};


const RUNTIME_NAMES: Record<string, string> = {
  deno: "Deno",
  node: "Node",
  quickjs: "QuickJS",
};


function Step({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div>
      <p className="text-2xs font-medium text-fg">{title}</p>
      <div className="mt-1 space-y-1.5 text-2xs leading-relaxed text-muted">{children}</div>
    </div>
  );
}


function LinkHelp({ links }: { links: LinkSupport }) {


  const [shown, setShown] = useState(!links.available);

  const platform = PLATFORM_NAMES[links.platform];
  const stale = links.versionAgeDays !== null && links.versionAgeDays > links.staleAfterDays;

  const nightly = links.nightlyCommand ? (
    <span className="mt-2 block">
      <Copyable text={links.nightlyCommand} />
    </span>
  ) : null;

  return (
    <div className="mt-2 space-y-2">
      {links.available ? (
        <p className="truncate text-2xs text-faint" title={links.path ?? undefined}>
          yt-dlp {links.version ?? "is installed"}
          {links.path ? ` · ${links.path}` : ""}
        </p>
      ) : (
        <Callout tone="neutral" title="Links need yt-dlp">
          ZScribe does not bundle it: yt-dlp has to be updated whenever a site changes its player,
          so a copy frozen into a release would break and stay broken. Install it and this box
          starts working.
        </Callout>
      )}


      {stale ? (
        <Callout tone="warning" title={`This copy of yt-dlp is ${links.versionAgeDays} days old`}>
          YouTube usually turns away a build that far behind its player, with a 403 that reads like
          a broken link. The nightly build carries the fix, and it goes beside the copy you have
          rather than over it.
          {nightly}
        </Callout>
      ) : null}

      <button
        type="button"
        onClick={() => setShown((was) => !was)}
        aria-expanded={shown}
        className={cn(
          "-mx-1 flex items-center gap-1 rounded px-1 py-0.5",
          "text-2xs text-muted transition-colors duration-fast ease-out hover:text-fg",
          "[&_svg]:size-3 [&_svg]:text-faint",
        )}
      >
        {shown ? <ChevronDown /> : <ChevronRight />}
        How links work on {platform}
      </button>

      {shown ? (
        <div className="space-y-3.5 rounded-lg border border-line-subtle bg-inset/40 px-3 py-3">
          {!links.available ? (
            <Step title={`Install it on ${platform}`}>
              <p>The command that fits what is already on this machine:</p>
              <Copyable text={links.installCommand} />
            </Step>
          ) : null}

          <Step title="Keep it new, or YouTube says no">
            <p>
              YouTube changes its player every few weeks and then refuses the audio to any yt-dlp
              that has fallen behind — a 403 here is that, not the link and not the connection. The
              nightly build gets the fix first. ZScribe looks in its own directory before anything
              on PATH, so this sits beside a package manager&rsquo;s copy instead of fighting it.
            </p>
            {links.nightlyCommand ? <Copyable text={links.nightlyCommand} /> : null}
          </Step>

          <Step title="YouTube also needs a JavaScript runtime">
            <p>
              It hands the audio over only after a JavaScript challenge, which yt-dlp solves with
              Deno, Node or QuickJS. It finds Deno by itself; ZScribe names the other two for it.
            </p>
            {links.jsRuntime ? (
              <p className="flex items-center gap-1.5 text-fg [&_svg]:size-3 [&_svg]:text-success">
                <CircleCheck />
                {RUNTIME_NAMES[links.jsRuntime] ?? links.jsRuntime} is installed
                {links.jsRuntime === "deno" ? "" : ", and ZScribe tells yt-dlp to use it"}.
              </p>
            ) : (
              <>
                <p className="flex items-center gap-1.5 text-fg [&_svg]:size-3 [&_svg]:text-warning">
                  <CircleAlert />
                  None of the three is here, so YouTube will fail on the challenge.
                </p>
                <Copyable text={links.jsRuntimeCommand} />
              </>
            )}
          </Step>

          <Step title="No Python, and no virtual environment">
            <p>
              The <span className="font-mono text-fg">{links.standaloneAsset}</span> build from the
              yt-dlp releases page carries its own interpreter, so nothing else has to be installed.
              Save it under this exact name — ZScribe looks for that one, not for the name it
              downloads as — and it wins over every other copy
              {links.platform === "windows" ? "" : ", once chmod +x has been run on it"}:
            </p>
            <Copyable text={links.toolsPath} />
            {links.platform === "macos" ? (
              <>
                <p>
                  Downloaded through a browser it arrives quarantined, and Gatekeeper refuses to run
                  an unsigned binary. Clearing that is a one-off:
                </p>
                <Copyable text={`xattr -d com.apple.quarantine "${links.toolsPath}"`} />
              </>
            ) : null}
          </Step>
        </div>
      ) : null}
    </div>
  );
}


export function ImportButton() {
  const [open_, setOpen] = useState(false);
  const [url, setUrl] = useState("");
  const [busy, setBusy] = useState<null | "file" | "link">(null);
  const [percent, setPercent] = useState<number | null>(null);
  const [error, setError] = useState<CommandError | null>(null);
  const [links, setLinks] = useState<LinkSupport | null>(null);

  const refreshRecordings = useAppStore((state) => state.refreshRecordings);
  const status = useAppStore((state) => state.status);


  const eta = useEta(percent, busy ?? "");


  useEffect(() => {
    if (!open_) return;
    void ipc
      .linkSupport()
      .then(setLinks)
      .catch(() => setLinks(null));
  }, [open_]);


  useEffect(() => {
    const unlisten = on<StageProgress>(EVENTS.transcribeProgress, (event) => {
      if (event.stage === "importing") setPercent(event.percent);
    });

    return () => {
      void unlisten.then((off) => off());
    };
  }, []);

  const linksAvailable = links?.available ?? null;

  const run = async (what: "file" | "link", action: () => Promise<string>) => {
    setError(null);
    setPercent(null);
    setBusy(what);
    try {
      await action();
      setOpen(false);
      setUrl("");
      await refreshRecordings();
    } catch (raw) {
      setError(toCommandError(raw));


      setOpen(true);
    } finally {
      setBusy(null);
      setPercent(null);
    }
  };

  const chooseFile = async () => {
    try {
      const extensions = await ipc.importableExtensions();
      const chosen = await open({
        multiple: false,
        filters: [{ name: "Audio and video", extensions }],
      });
      if (typeof chosen !== "string") return;

      await run("file", () => ipc.importFile(chosen));
    } catch (raw) {


      setError(toCommandError(raw));
    }
  };


  const busyRef = useRef(busy);
  useEffect(() => {
    busyRef.current = busy;
  }, [busy]);


  useEffect(() => {
    const unlisten = getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type !== "drop" || busyRef.current) return;

      const [first] = event.payload.paths;
      if (first) void run("file", () => ipc.importFile(first));
    });

    return () => {
      void unlisten.then((off) => off());
    };

  }, []);


  const working = (
    <div className="mt-2 space-y-1">
      <div className="h-1 overflow-hidden rounded-full bg-inset">
        {percent === null ? (
          <div className="h-full w-1/3 rounded-full bg-accent motion-safe:animate-indeterminate" />
        ) : (
          <div
            className="h-full rounded-full bg-accent transition-[width] duration-base ease-out"
            style={{ width: `${percent}%` }}
          />
        )}
      </div>
      <p data-numeric className="text-2xs text-faint">
        {percent === null
          ? busy === "link"
            ? "Reading the link…"
            : "Reading the file…"
          : `${busy === "link" ? "Downloading the audio" : "Reading the file"} — ${percent}%${
              eta ? ` · ${eta}` : ""
            }`}
        . You can close this window; it carries on.
      </p>
    </div>
  );

  return (
    <>
      <button
        type="button"
        onClick={() => setOpen(true)}
        disabled={status.active}
        title={status.active ? "Stop the recording first" : "Import a file or a link"}
        aria-label="Import a file or a link"
        className={cn(
          "inline-flex h-7 items-center gap-2 rounded-md px-2.5",
          "text-xs font-medium text-muted transition-colors duration-fast ease-out",
          "hover:bg-hover hover:text-fg disabled:pointer-events-none disabled:opacity-40",
          "[&_svg]:size-3.5",
        )}
      >
        <Import />
        Import
      </button>

      <Dialog
        open={open_}


        onOpenChange={(next) => {
          setOpen(next);
          if (!next) setError(null);
        }}
        title="Import a recording"
        description="Audio or video from this machine, or a link. It is transcribed and summarised exactly like something you recorded here."
      >
        <div className="space-y-5">
          <div>
            <Button
              variant="primary"
              disabled={busy !== null}
              onClick={() => void chooseFile()}
              className="w-full"
            >
              {busy === "file" ? <Loader2 className="animate-spin" /> : <FileAudio />}
              {busy === "file" ? "Importing…" : "Choose a file…"}
            </Button>
            {busy === "file" ? (
              working
            ) : (
              <p className="mt-1.5 text-2xs leading-relaxed text-faint">
                WAV, MP3, M4A, MP4, FLAC, OGG, MKV and AIFF. Only the sound is used, so a video is
                fine. You can also drop a file onto this window.
              </p>
            )}
          </div>

          <div className="flex items-center gap-3">
            <span className="h-px flex-1 bg-line-subtle" />
            <span className="text-2xs text-faint">or</span>
            <span className="h-px flex-1 bg-line-subtle" />
          </div>

          <div>
            <div className="flex items-center gap-2">
              <Input
                value={url}
                onChange={(event) => setUrl(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter" && url.trim() && !busy) {
                    void run("link", () => ipc.importLink(url.trim()));
                  }
                }}
                placeholder="https://www.youtube.com/watch?v=…"
                aria-label="Link to import"
                spellCheck={false}
                disabled={linksAvailable === false || busy !== null}
                className="flex-1"
              />
              <Button
                disabled={!url.trim() || busy !== null || linksAvailable === false}
                onClick={() => void run("link", () => ipc.importLink(url.trim()))}
              >
                {busy === "link" ? <Loader2 className="animate-spin" /> : <Link2 />}
                {busy === "link" ? "Fetching…" : "Fetch"}
              </Button>
            </div>

            {busy === "link" ? working : null}

            <p className="mt-1.5 text-2xs leading-relaxed text-faint">
              Only the audio is downloaded. Import what you have the right to — a link being public
              is not the same as it being yours to keep.
            </p>

            {links ? <LinkHelp links={links} /> : null}
          </div>

          {error ? (
            <Callout tone="danger" title={error.message}>
              {error.remedy}
            </Callout>
          ) : null}
        </div>
      </Dialog>
    </>
  );
}
