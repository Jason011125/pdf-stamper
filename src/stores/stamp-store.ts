import { create } from 'zustand';

export type StampType = 'image' | 'text';

interface StampStore {
  type: StampType;
  imagePath: string | null;
  imagePreviewUrl: string | null;
  naturalWidthPx: number;
  naturalHeightPx: number;
  scalePercent: number;
  rotationDeg: number;
  text: string;
  fontFamily: string;
  fontSize: number;
  color: string;
  xPt: number;
  yPt: number;
  widthPt: number;
  heightPt: number;
  isPlaced: boolean;
  isExporting: boolean;
  exportProgress: { current: number; total: number };

  setType: (type: StampType) => void;
  setImage: (path: string, previewUrl: string) => void;
  setImageWithDimensions: (
    path: string,
    previewUrl: string,
    naturalW: number,
    naturalH: number,
  ) => void;
  clearImage: () => void;
  setScalePercent: (pct: number) => void;
  setRotation: (deg: number) => void;
  setText: (text: string) => void;
  setFontFamily: (fontFamily: string) => void;
  setFontSize: (fontSize: number) => void;
  setColor: (color: string) => void;
  setPosition: (xPt: number, yPt: number) => void;
  setSize: (widthPt: number, heightPt: number) => void;
  setPlaced: (placed: boolean) => void;
  setExporting: (exporting: boolean) => void;
  setExportProgress: (current: number, total: number) => void;
}

function derivePts(naturalW: number, naturalH: number, pct: number): { widthPt: number; heightPt: number } {
  const scale = pct / 100;
  return {
    widthPt: Math.max(1, naturalW * scale),
    heightPt: Math.max(1, naturalH * scale),
  };
}

export const useStampStore = create<StampStore>((set) => ({
  type: 'image',
  imagePath: null,
  imagePreviewUrl: null,
  naturalWidthPx: 0,
  naturalHeightPx: 0,
  scalePercent: 100,
  rotationDeg: 0,
  text: 'STAMP',
  fontFamily: 'Helvetica',
  fontSize: 24,
  color: '#ff0000',
  xPt: 0,
  yPt: 0,
  widthPt: 100,
  heightPt: 100,
  isPlaced: false,
  isExporting: false,
  exportProgress: { current: 0, total: 0 },

  setType: (type) => set({ type }),
  setImage: (path, previewUrl) => set({ imagePath: path, imagePreviewUrl: previewUrl }),
  setImageWithDimensions: (path, previewUrl, naturalW, naturalH) =>
    set((state) => ({
      imagePath: path,
      imagePreviewUrl: previewUrl,
      naturalWidthPx: naturalW,
      naturalHeightPx: naturalH,
      ...derivePts(naturalW, naturalH, state.scalePercent),
    })),
  clearImage: () =>
    set((state) => {
      if (state.imagePreviewUrl) URL.revokeObjectURL(state.imagePreviewUrl);
      return {
        imagePath: null,
        imagePreviewUrl: null,
        naturalWidthPx: 0,
        naturalHeightPx: 0,
      };
    }),
  setScalePercent: (pct) =>
    set((state) => ({
      scalePercent: pct,
      ...derivePts(state.naturalWidthPx, state.naturalHeightPx, pct),
    })),
  setRotation: (deg) => set({ rotationDeg: Math.round(((deg % 360) + 360) % 360) }),
  setText: (text) => set({ text }),
  setFontFamily: (fontFamily) => set({ fontFamily }),
  setFontSize: (fontSize) => set({ fontSize }),
  setColor: (color) => set({ color }),
  setPosition: (xPt, yPt) => set({ xPt, yPt }),
  setSize: (widthPt, heightPt) => set({ widthPt, heightPt }),
  setPlaced: (placed) => set({ isPlaced: placed }),
  setExporting: (exporting) => set({ isExporting: exporting }),
  setExportProgress: (current, total) =>
    set({ exportProgress: { current, total } }),
}));
