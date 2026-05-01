import { create } from 'zustand';
import { usePdfStore } from './pdf-store';

export type StampType = 'image' | 'text';

/** All user-configurable stamp fields. Lives in either `defaultConfig`
 * (applies to every loaded PDF) or as a `Partial<StampConfig>` in
 * `overrides[pdfId]` (per-PDF override that wins over default). */
export interface StampConfig {
  type: StampType;
  imagePath: string | null;
  imagePreviewUrl: string | null;
  text: string;
  fontFamily: string;
  fontSize: number;
  color: string;
  xPt: number;
  yPt: number;
  widthPt: number;
  heightPt: number;
  lockAspect: boolean;
  /** Aspect ratio (widthPt / heightPt) preserved while lockAspect is on.
   * Stored explicitly so it survives intermediate 0 / invalid values the
   * user may type while editing W/H. */
  lockedAspect: number;
  /** User-chosen stamp rotation in degrees CCW around the stamp's center.
   * Normalized to [0, 360) when written via setDefault/setOverride. */
  rotationDeg: number;
}

export const DEFAULT_STAMP_CONFIG: StampConfig = {
  type: 'image',
  imagePath: null,
  imagePreviewUrl: null,
  text: 'STAMP',
  fontFamily: 'Helvetica',
  fontSize: 24,
  color: '#ff0000',
  xPt: 0,
  yPt: 0,
  widthPt: 100,
  heightPt: 100,
  lockAspect: true,
  lockedAspect: 1,
  rotationDeg: 0,
};

function normalizeRotation(deg: number): number {
  if (!Number.isFinite(deg)) return 0;
  return ((deg % 360) + 360) % 360;
}

/** rotationDeg is the only field with normalization rules; route partials
 * through this helper so setDefault and setOverride share the same logic. */
function normalizePartial(partial: Partial<StampConfig>): Partial<StampConfig> {
  if (partial.rotationDeg === undefined) return partial;
  return { ...partial, rotationDeg: normalizeRotation(partial.rotationDeg) };
}

interface StampStore {
  defaultConfig: StampConfig;
  overrides: Record<string, Partial<StampConfig>>;

  isPlaced: boolean;
  isExporting: boolean;
  exportProgress: { current: number; total: number };

  /** Returns the merged StampConfig for `pdfId`: default fields with any
   * override fields layered on top. Override-less PDFs get a copy of default. */
  getEffectiveConfig: (pdfId: string) => StampConfig;

  setDefault: (partial: Partial<StampConfig>) => void;
  setOverride: (pdfId: string, partial: Partial<StampConfig>) => void;
  clearOverride: (pdfId: string) => void;
  /** Routes a position/size/rotation edit from the UI. Uses a purely
   * index-based rule: edits on the master PDF (files[0] in pdf-store) write
   * to defaultConfig; edits on any other PDF write to overrides[pdfId].
   * If `pdfId` is empty (no PDFs loaded), the call is a no-op. Keeps the
   * default-vs-override decision in one place so components stay mechanical. */
  applyEdit: (pdfId: string, partial: Partial<StampConfig>) => void;

  setPlaced: (placed: boolean) => void;
  setExporting: (exporting: boolean) => void;
  setExportProgress: (current: number, total: number) => void;

  // Shim setters — write to defaultConfig. Used for global stamp setup
  // (image / text / color / font / lockAspect) where per-PDF overrides don't
  // make sense. Per-PDF position/size/rotation edits go through applyEdit.
  setType: (type: StampType) => void;
  setImage: (path: string, previewUrl: string) => void;
  clearImage: () => void;
  setText: (text: string) => void;
  setFontFamily: (fontFamily: string) => void;
  setFontSize: (fontSize: number) => void;
  setColor: (color: string) => void;
  setPosition: (xPt: number, yPt: number) => void;
  setSize: (widthPt: number, heightPt: number) => void;
  setLockAspect: (locked: boolean) => void;
  setLockedAspect: (aspect: number) => void;
  setRotationDeg: (deg: number) => void;
}

export const useStampStore = create<StampStore>((set, get) => ({
  defaultConfig: { ...DEFAULT_STAMP_CONFIG },
  overrides: {},
  isPlaced: false,
  isExporting: false,
  exportProgress: { current: 0, total: 0 },

  getEffectiveConfig: (pdfId) => {
    const { defaultConfig, overrides } = get();
    const override = overrides[pdfId];
    return override ? { ...defaultConfig, ...override } : { ...defaultConfig };
  },

  setDefault: (partial) =>
    set((state) => ({
      defaultConfig: { ...state.defaultConfig, ...normalizePartial(partial) },
    })),

  setOverride: (pdfId, partial) =>
    set((state) => ({
      overrides: {
        ...state.overrides,
        [pdfId]: {
          ...(state.overrides[pdfId] ?? {}),
          ...normalizePartial(partial),
        },
      },
    })),

  clearOverride: (pdfId) =>
    set((state) => {
      if (!(pdfId in state.overrides)) return state;
      const next = { ...state.overrides };
      delete next[pdfId];
      return { overrides: next };
    }),

  applyEdit: (pdfId, partial) =>
    set((state) => {
      if (!pdfId) return state;
      const normalized = normalizePartial(partial);
      const masterId = usePdfStore.getState().files[0]?.path;
      if (pdfId === masterId) {
        return {
          defaultConfig: { ...state.defaultConfig, ...normalized },
        };
      }
      return {
        overrides: {
          ...state.overrides,
          [pdfId]: { ...(state.overrides[pdfId] ?? {}), ...normalized },
        },
      };
    }),

  setPlaced: (placed) => set({ isPlaced: placed }),
  setExporting: (exporting) => set({ isExporting: exporting }),
  setExportProgress: (current, total) =>
    set({ exportProgress: { current, total } }),

  setType: (type) => get().setDefault({ type }),
  setImage: (path, previewUrl) =>
    get().setDefault({ imagePath: path, imagePreviewUrl: previewUrl }),
  clearImage: () => {
    const { defaultConfig } = get();
    if (defaultConfig.imagePreviewUrl) {
      URL.revokeObjectURL(defaultConfig.imagePreviewUrl);
    }
    get().setDefault({ imagePath: null, imagePreviewUrl: null });
  },
  setText: (text) => get().setDefault({ text }),
  setFontFamily: (fontFamily) => get().setDefault({ fontFamily }),
  setFontSize: (fontSize) => get().setDefault({ fontSize }),
  setColor: (color) => get().setDefault({ color }),
  setPosition: (xPt, yPt) => get().setDefault({ xPt, yPt }),
  setSize: (widthPt, heightPt) => get().setDefault({ widthPt, heightPt }),
  setLockAspect: (locked) => get().setDefault({ lockAspect: locked }),
  setLockedAspect: (aspect) => get().setDefault({ lockedAspect: aspect }),
  setRotationDeg: (deg) => get().setDefault({ rotationDeg: deg }),
}));
