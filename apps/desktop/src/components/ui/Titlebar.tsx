import { useEffect, useState, type ReactNode } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, Copy, X } from "lucide-react";
import { cn } from "@/lib/cn";


const IS_MAC = typeof navigator !== "undefined" && /Mac/i.test(navigator.userAgent);

export function Titlebar({ children }: { children?: ReactNode }) {
  const [maximized, setMaximized] = useState(false);
  const appWindow = getCurrentWindow();

  useEffect(() => {
    let cancelled = false;
    const sync = () => {
      void appWindow.isMaximized().then((value) => {
        if (!cancelled) setMaximized(value);
      });
    };
    sync();
    const unlisten = appWindow.onResized(sync);
    return () => {
      cancelled = true;
      void unlisten.then((off) => off());
    };
  }, [appWindow]);


  const controls = (
    <div className="flex items-center">
      {IS_MAC ? (
        <WindowButton label="Close" danger onClick={() => void appWindow.close()}>
          <X />
        </WindowButton>
      ) : null}
      <WindowButton label="Minimize" onClick={() => void appWindow.minimize()}>
        <Minus />
      </WindowButton>
      <WindowButton
        label={maximized ? "Restore" : IS_MAC ? "Zoom" : "Maximize"}
        onClick={() => void appWindow.toggleMaximize()}
      >
        {maximized ? <Copy className="scale-x-[-1]" /> : <Square />}
      </WindowButton>
      {IS_MAC ? null : (
        <WindowButton label="Close" danger onClick={() => void appWindow.close()}>
          <X />
        </WindowButton>
      )}
    </div>
  );

  return (
    <header
      data-tauri-drag-region
      className={cn(
        "flex h-9 shrink-0 items-center gap-2",
        IS_MAC ? "pr-3 pl-1" : "pr-1 pl-3",
        "border-b border-line-subtle bg-canvas/60 select-none",
      )}
    >
      {IS_MAC ? controls : null}


      <span
        data-tauri-drag-region
        className="pointer-events-none text-2xs font-medium tracking-wide text-faint"
      >
        ZScribe
      </span>

      <div className="flex-1" data-tauri-drag-region />

      {children ? <div className="flex items-center gap-2">{children}</div> : null}

      {IS_MAC ? null : controls}
    </header>
  );
}

function WindowButton({
  label,
  onClick,
  danger,
  children,
}: {
  label: string;
  onClick: () => void;
  danger?: boolean;
  children: ReactNode;
}) {
  return (
    <button
      aria-label={label}
      title={label}
      onClick={onClick}
      className={cn(
        "inline-flex h-7 w-9 items-center justify-center rounded-md",
        "text-faint transition-colors duration-fast ease-out",
        "[&_svg]:size-3.5",
        danger ? "hover:bg-danger hover:text-white" : "hover:bg-hover hover:text-fg",
      )}
    >
      {children}
    </button>
  );
}


export function ResizeEdges() {
  const appWindow = getCurrentWindow();
  const start = (direction: string) => () => {
    void appWindow.startResizeDragging(direction as never);
  };

  return (
    <>

      <div onMouseDown={start("North")} className="fixed inset-x-2 top-0 z-50 h-1 cursor-ns-resize" />
      <div onMouseDown={start("South")} className="fixed inset-x-2 bottom-0 z-50 h-1 cursor-ns-resize" />
      <div onMouseDown={start("West")} className="fixed inset-y-2 left-0 z-50 w-1 cursor-ew-resize" />
      <div onMouseDown={start("East")} className="fixed inset-y-2 right-0 z-50 w-1 cursor-ew-resize" />

      <div onMouseDown={start("NorthWest")} className="fixed top-0 left-0 z-50 size-2 cursor-nwse-resize" />
      <div onMouseDown={start("NorthEast")} className="fixed top-0 right-0 z-50 size-2 cursor-nesw-resize" />
      <div onMouseDown={start("SouthWest")} className="fixed bottom-0 left-0 z-50 size-2 cursor-nesw-resize" />
      <div onMouseDown={start("SouthEast")} className="fixed right-0 bottom-0 z-50 size-2 cursor-nwse-resize" />
    </>
  );
}
