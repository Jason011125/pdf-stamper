import { create } from 'zustand';

export interface StampPos {
  xPt: number;
  yPt: number;
}

export interface PdfFile {
  path: string;
  filename: string;
  widthPt: number;
  heightPt: number;
  previewUrl: string | null;
  /** null = not individually placed; inherits global stamp position at export */
  stampPos: StampPos | null;
}

interface PdfStore {
  files: PdfFile[];
  selectedIndex: number;
  addFiles: (files: PdfFile[]) => void;
  setSelectedIndex: (index: number) => void;
  setPreviewUrl: (index: number, url: string) => void;
  setStampPos: (index: number, xPt: number, yPt: number) => void;
  removeFile: (index: number) => void;
  clearFiles: () => void;
}

export const usePdfStore = create<PdfStore>((set) => ({
  files: [],
  selectedIndex: 0,

  addFiles: (files) =>
    set((state) => ({
      files: [...state.files, ...files],
    })),

  setSelectedIndex: (index) => set({ selectedIndex: index }),

  setPreviewUrl: (index, url) =>
    set((state) => ({
      files: state.files.map((f, i) =>
        i === index ? { ...f, previewUrl: url } : f,
      ),
    })),

  setStampPos: (index, xPt, yPt) =>
    set((state) => ({
      files: state.files.map((f, i) =>
        i === index ? { ...f, stampPos: { xPt, yPt } } : f,
      ),
    })),

  removeFile: (index) =>
    set((state) => {
      const prev = state.files[index]?.previewUrl;
      if (prev) URL.revokeObjectURL(prev);
      const files = state.files.filter((_, i) => i !== index);
      const selectedIndex = Math.min(state.selectedIndex, Math.max(0, files.length - 1));
      return { files, selectedIndex };
    }),

  clearFiles: () =>
    set((state) => {
      for (const f of state.files) {
        if (f.previewUrl) URL.revokeObjectURL(f.previewUrl);
      }
      return { files: [], selectedIndex: 0 };
    }),
}));
