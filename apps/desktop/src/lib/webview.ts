import { invoke } from "@tauri-apps/api/core";

const RELOAD_KEYS = new Set(["r", "R", "F5"]);


function reportFailures() {
  const report = (message: string, source: string) => {
    void invoke("report_error", { message, source }).catch(() => {});
  };

  window.addEventListener("error", (event) => {
    const where = event.filename ? `${event.filename}:${event.lineno}` : "window";
    report(event.error?.stack ?? event.message, where);
  });

  window.addEventListener("unhandledrejection", (event) => {
    const reason = event.reason;
    report(reason?.stack ?? String(reason?.message ?? reason), "promise");
  });
}

export function lockDownWebview() {


  reportFailures();

  if (import.meta.env.DEV) return;

  window.addEventListener("contextmenu", (event) => {
    if (isEditable(event.target)) return;
    event.preventDefault();
  });

  window.addEventListener(
    "keydown",
    (event) => {
      if (event.key === "F5" || ((event.ctrlKey || event.metaKey) && RELOAD_KEYS.has(event.key))) {
        event.preventDefault();
        return;
      }

      if ((event.ctrlKey || event.metaKey) && (event.key === "p" || event.key === "P")) {
        event.preventDefault();
      }
    },
    true,
  );

  window.addEventListener("dragover", (event) => event.preventDefault());
  window.addEventListener("drop", (event) => event.preventDefault());
}

function isEditable(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false;

  const element = target.closest("input, textarea, [contenteditable=''], [contenteditable='true'], [data-selectable]");
  if (!element) return false;

  if (element instanceof HTMLInputElement) return !element.disabled && !element.readOnly;
  if (element instanceof HTMLTextAreaElement) return !element.disabled && !element.readOnly;

  return true;
}
