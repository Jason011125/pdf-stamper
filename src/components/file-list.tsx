import { useCallback, useRef } from 'react';
import { usePdfStore } from '../stores/pdf-store';
import { openPdfDialog, loadPdfs, renderPage } from '../services/pdf-bridge';

const PREVIEW_WIDTH = 2000;

export function FileList(): React.JSX.Element {
  const files = usePdfStore((s) => s.files);
  const selectedIndex = usePdfStore((s) => s.selectedIndex);
  const addFiles = usePdfStore((s) => s.addFiles);
  const setSelectedIndex = usePdfStore((s) => s.setSelectedIndex);
  const removeFile = usePdfStore((s) => s.removeFile);

  const isLoadingRef = useRef(false);

  const handleOpen = useCallback(async () => {
    if (isLoadingRef.current) return;
    isLoadingRef.current = true;

    try {
      const paths = await openPdfDialog();
      if (!paths || paths.length === 0) return;

      const infos = await loadPdfs(paths);

      const pdfFiles = infos.map((info) => ({
        path: info.path,
        filename: info.filename,
        widthPt: info.width_pt,
        heightPt: info.height_pt,
        previewUrl: null,
        stampPos: null,
      }));

      addFiles(pdfFiles);

      const startIndex = usePdfStore.getState().files.length - pdfFiles.length;
      setSelectedIndex(startIndex);

      // Render previews in background — errors on individual files don't break the batch
      for (let i = 0; i < pdfFiles.length; i++) {
        const file = pdfFiles[i];
        if (file) {
          try {
            const url = await renderPage(file.path, PREVIEW_WIDTH);
            usePdfStore.getState().setPreviewUrl(startIndex + i, url);
          } catch {
            // Skip failed renders — file stays in list with null preview
          }
        }
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      alert(`Failed to open PDFs: ${message}`);
    } finally {
      isLoadingRef.current = false;
    }
  }, [addFiles, setSelectedIndex]);

  return (
    <div className="flex flex-col h-full">
      <div className="flex items-center justify-between mb-3">
        <h2 className="text-sm font-semibold text-gray-500 uppercase tracking-wide">
          Files
        </h2>
        <span className="text-xs text-gray-400">{files.length}</span>
      </div>

      <button
        onClick={handleOpen}
        className="mb-3 w-full rounded-md bg-blue-500 px-3 py-2 text-sm font-medium text-white hover:bg-blue-600 transition-colors"
      >
        Open PDFs
      </button>

      <div className="flex-1 overflow-y-auto space-y-1">
        {files.length === 0 && (
          <p className="text-sm text-gray-400 italic">No files loaded</p>
        )}

        {files.map((file, index) => (
          <div
            key={file.path}
            onClick={() => setSelectedIndex(index)}
            className={`flex items-center gap-2 rounded-md px-2 py-1.5 text-sm cursor-pointer transition-colors ${
              index === selectedIndex
                ? 'bg-blue-50 text-blue-700'
                : 'text-gray-700 hover:bg-gray-100'
            }`}
          >
            <span className="flex-1 truncate">{file.filename}</span>
            <button
              onClick={(e) => {
                e.stopPropagation();
                removeFile(index);
              }}
              className="text-gray-400 hover:text-red-500 transition-colors"
              title="Remove"
            >
              &times;
            </button>
          </div>
        ))}
      </div>
    </div>
  );
}
