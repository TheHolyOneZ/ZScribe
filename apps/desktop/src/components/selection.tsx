import { useState } from "react";
import { Copy } from "lucide-react";

import { MenuItem, MenuSeparator } from "@/components/ui";

export const copyText = (text: string) => void navigator.clipboard.writeText(text);


export function useSelection() {
  const [selection, setSelection] = useState("");
  const capture = () => setSelection(window.getSelection()?.toString().trim() ?? "");
  return { selection, capture };
}

export function CopySelection({ selection }: { selection: string }) {
  if (!selection) return null;
  return (
    <>
      <MenuItem icon={<Copy />} onSelect={() => copyText(selection)}>
        Copy selection
      </MenuItem>
      <MenuSeparator />
    </>
  );
}
