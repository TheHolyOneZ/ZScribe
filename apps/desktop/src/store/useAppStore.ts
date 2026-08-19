import { create } from "zustand";

import {
  ipc,
  type AppSettings,
  type Capabilities,
  type Recording,
  type RecordingStatus,
  type StageProgress,
  type Template,
} from "@/lib/ipc";

interface AppState {
  loading: boolean;
  settings: AppSettings | null;
  capabilities: Capabilities | null;
  templates: Template[];

  recordings: Recording[];
  selectedId: string | null;


  status: RecordingStatus;


  progress: StageProgress | null;

  hydrate: () => Promise<void>;
  refreshRecordings: () => Promise<void>;
  update: (patch: Partial<AppSettings>) => Promise<void>;
  select: (id: string | null) => void;
  setStatus: (status: RecordingStatus) => void;
  setProgress: (progress: StageProgress | null) => void;
}

const IDLE: RecordingStatus = {
  active: false,
  paused: false,
  durationMs: 0,
  level: { rms: 0, peak: 0 },
  problem: null,
  rewindMs: 0,
};

const RECORDING_LIMIT = 500;

export const useAppStore = create<AppState>((set, get) => ({
  loading: true,
  settings: null,
  capabilities: null,
  templates: [],
  recordings: [],
  selectedId: null,
  status: IDLE,
  progress: null,

  hydrate: async () => {
    const [settings, capabilities, templates, recordings, status] = await Promise.all([
      ipc.getSettings(),
      ipc.getCapabilities(),
      ipc.getTemplates(),
      ipc.listRecordings(RECORDING_LIMIT),
      ipc.recordingStatus(),
    ]);

    set((state) => ({
      settings,
      capabilities,
      templates,
      recordings,
      status,
      loading: false,


      selectedId: state.selectedId ?? recordings[0]?.id ?? null,
    }));
  },

  refreshRecordings: async () => {
    const recordings = await ipc.listRecordings(RECORDING_LIMIT);
    set((state) => ({
      recordings,
      selectedId:
        state.selectedId && recordings.some((r) => r.id === state.selectedId)
          ? state.selectedId
          : (recordings[0]?.id ?? null),
    }));
  },

  update: async (patch) => {
    const current = get().settings;
    if (!current) return;


    const optimistic = { ...current, ...patch };
    set({ settings: optimistic });

    try {
      set({ settings: await ipc.saveSettings(optimistic) });
    } catch (error) {
      set({ settings: current });
      throw error;
    }
  },

  select: (selectedId) => set({ selectedId }),
  setStatus: (status) => set({ status }),
  setProgress: (progress) => set({ progress }),
}));
