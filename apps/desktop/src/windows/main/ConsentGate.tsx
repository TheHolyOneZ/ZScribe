import { useEffect, useState } from "react";

import { Button, Dialog } from "@/components/ui";
import { useAppStore } from "@/store/useAppStore";


export function ConsentGate() {
  const settings = useAppStore((state) => state.settings);
  const update = useAppStore((state) => state.update);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    if (settings && !settings.consentAcknowledged) setOpen(true);
  }, [settings]);

  const acknowledge = () => {
    setOpen(false);
    void update({ consentAcknowledged: true });
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {


        if (!next) setOpen(false);
      }}
      title="Before you record"
      description="One thing worth knowing, and then ZScribe will not mention it again."
      footer={
        <Button variant="primary" onClick={acknowledge}>
          I understand
        </Button>
      }
    >
      <div className="space-y-3 text-sm leading-relaxed text-muted">
        <p>
          Recording a conversation without the knowledge of the people in it is illegal in many
          places. In Germany it is a criminal offence under <strong className="text-fg">§ 201 StGB</strong>,
          and similar rules apply across the EU and in most of the United States.
        </p>
        <p>
          Tell people they are being recorded, and get their agreement. ZScribe can play a tone when
          recording starts and add a note to the transcript — both are in Audio sources.
        </p>
        <p>
          While ZScribe is recording, the tray icon says so. That indicator cannot be switched off.
        </p>
        <p className="text-xs text-faint">
          None of this is legal advice, and none of your audio leaves this computer.
        </p>
      </div>
    </Dialog>
  );
}
