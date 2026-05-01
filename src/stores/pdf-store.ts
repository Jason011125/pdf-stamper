import { create } from 'zustand';

export interface PdfFile {
  path: string;
  filename: string;
  widthPt: number;
  heightPt: number;
  previewUrl: string | null;
}

interface PdfStore {
  files: PdfFile[];
  selectedIndex: number;
  addFiles: (files: PdfFile[]) => void;
  setSelectedIndex: (index: number) => void;
  setPreviewUrl: (index: number, url: string) => void;
  /** Remove a file by pdfId (path). Revokes its previewUrl. Selection rule:
   * if the removed PDF was currently selected, jump to the first remaining
   * PDF (index 0); otherwise track the same selected file (decrement index
   * when the removed one was before it). Lookup by id avoids stale-index
   * bugs from rapid clicks during reorders / concurrent removals. */
  removeFile: (pdfId: string) => void;
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

  removeFile: (pdfId) =>
    set((state) => {
      const idx = state.files.findIndex((f) => f.path === pdfId);
      if (idx === -1) return state;

      const removed = state.files[idx];
      if (removed?.previewUrl) URL.revokeObjectURL(removed.previewUrl);

      const files = state.files.filter((_, i) => i !== idx);

      let selectedIndex = state.selectedIndex;
      if (idx === state.selectedIndex) {
        selectedIndex = 0;
      } else if (idx < state.selectedIndex) {
        selectedIndex = state.selectedIndex - 1;
      }
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
