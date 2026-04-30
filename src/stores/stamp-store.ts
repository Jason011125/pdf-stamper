import { create } from 'zustand';

export type StampType = 'image' | 'text';

interface StampStore {
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
   * user may type while editing W/H — deriving aspect from current
   * widthPt/heightPt would corrupt to NaN or 0/Infinity in that window. */
  lockedAspect: number;
  /** User-chosen stamp rotation in degrees CCW around the stamp's center.
   * Normalized to [0, 360) by `setRotationDeg`. */
  rotationDeg: number;
  isPlaced: boolean;
  isExporting: boolean;
  exportProgress: { current: number; total: number };

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
  setPlaced: (placed: boolean) => void;
  setExporting: (exporting: boolean) => void;
  setExportProgress: (current: number, total: number) => void;
}

export const useStampStore = create<StampStore>((set) => ({
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
  isPlaced: false,
  isExporting: false,
  exportProgress: { current: 0, total: 0 },

  setType: (type) => set({ type }),
  setImage: (path, previewUrl) => set({ imagePath: path, imagePreviewUrl: previewUrl }),
  clearImage: () =>
    set((state) => {
      if (state.imagePreviewUrl) URL.revokeObjectURL(state.imagePreviewUrl);
      return { imagePath: null, imagePreviewUrl: null };
    }),
  setText: (text) => set({ text }),
  setFontFamily: (fontFamily) => set({ fontFamily }),
  setFontSize: (fontSize) => set({ fontSize }),
  setColor: (color) => set({ color }),
  setPosition: (xPt, yPt) => set({ xPt, yPt }),
  setSize: (widthPt, heightPt) => set({ widthPt, heightPt }),
  setLockAspect: (locked) => set({ lockAspect: locked }),
  setLockedAspect: (aspect) => set({ lockedAspect: aspect }),
  setRotationDeg: (deg) => {
    if (!Number.isFinite(deg)) {
      set({ rotationDeg: 0 });
      return;
    }
    const normalized = ((deg % 360) + 360) % 360;
    set({ rotationDeg: normalized });
  },
  setPlaced: (placed) => set({ isPlaced: placed }),
  setExporting: (exporting) => set({ isExporting: exporting }),
  setExportProgress: (current, total) =>
    set({ exportProgress: { current, total } }),
}));
