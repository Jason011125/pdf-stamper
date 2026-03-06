import { useCallback } from 'react';
import { useStampStore, type StampType } from '../stores/stamp-store';
import { usePdfStore } from '../stores/pdf-store';
import { stampAllPdfs, selectOutputDir } from '../services/pdf-bridge';
import { open } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';

async function readImagePreview(
  path: string,
): Promise<{ previewUrl: string; naturalW: number; naturalH: number }> {
  const bytes = await invoke<number[]>('read_file_bytes', { path });
  const uint8 = new Uint8Array(bytes);
  const blob = new Blob([uint8], { type: 'image/png' });
  const previewUrl = URL.createObjectURL(blob);

  const { naturalW, naturalH } = await new Promise<{ naturalW: number; naturalH: number }>(
    (resolve) => {
      const img = new Image();
      img.onload = () => resolve({ naturalW: img.naturalWidth, naturalH: img.naturalHeight });
      img.onerror = () => resolve({ naturalW: 100, naturalH: 100 });
      img.src = previewUrl;
    },
  );

  return { previewUrl, naturalW, naturalH };
}

export function StampControls(): React.JSX.Element {
  const files = usePdfStore((s) => s.files);
  const selectedIndex = usePdfStore((s) => s.selectedIndex);

  const stampType = useStampStore((s) => s.type);
  const setType = useStampStore((s) => s.setType);
  const imagePreviewUrl = useStampStore((s) => s.imagePreviewUrl);
  const imagePath = useStampStore((s) => s.imagePath);
  const setImageWithDimensions = useStampStore((s) => s.setImageWithDimensions);
  const clearImage = useStampStore((s) => s.clearImage);
  const scalePercent = useStampStore((s) => s.scalePercent);
  const setScalePercent = useStampStore((s) => s.setScalePercent);
  const widthPt = useStampStore((s) => s.widthPt);
  const heightPt = useStampStore((s) => s.heightPt);
  const setSize = useStampStore((s) => s.setSize);
  const text = useStampStore((s) => s.text);
  const setText = useStampStore((s) => s.setText);
  const fontSize = useStampStore((s) => s.fontSize);
  const setFontSize = useStampStore((s) => s.setFontSize);
  const color = useStampStore((s) => s.color);
  const setColor = useStampStore((s) => s.setColor);
  const xPt = useStampStore((s) => s.xPt);
  const yPt = useStampStore((s) => s.yPt);
  const isPlaced = useStampStore((s) => s.isPlaced);
  const isExporting = useStampStore((s) => s.isExporting);
  const setExporting = useStampStore((s) => s.setExporting);
  const exportProgress = useStampStore((s) => s.exportProgress);
  const setExportProgress = useStampStore((s) => s.setExportProgress);
  const fontName = useStampStore((s) => s.fontFamily);

  const handleImageUpload = useCallback(async () => {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp'] }],
    });
    if (!selected) return;

    const path = typeof selected === 'string' ? selected : selected;
    const { previewUrl, naturalW, naturalH } = await readImagePreview(path);
    setImageWithDimensions(path, previewUrl, naturalW, naturalH);
  }, [setImageWithDimensions]);

  const handleApplyAll = useCallback(async () => {
    if (files.length === 0 || !isPlaced) return;

    const dir = await selectOutputDir();
    if (!dir) return;

    setExporting(true);
    setExportProgress(0, files.length);

    try {
      const paths = files.map((f) => f.path);
      const positions = files.map((f) => ({
        x: f.stampPos?.xPt ?? xPt,
        y: f.stampPos?.yPt ?? yPt,
      }));
      const result = await stampAllPdfs({
        paths,
        stampType,
        imagePath: stampType === 'image' ? imagePath : null,
        text: stampType === 'text' ? text : null,
        fontSize: stampType === 'text' ? fontSize : null,
        fontName: stampType === 'text' ? fontName : null,
        color: stampType === 'text' ? color : null,
        positions,
        width: widthPt,
        height: heightPt,
        outputDir: dir,
      });
      setExportProgress(result.length, files.length);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      alert(`Export failed: ${message}`);
    } finally {
      setExporting(false);
    }
  }, [
    files, isPlaced, stampType, imagePath, text, fontSize, fontName, color,
    xPt, yPt, widthPt, heightPt, setExporting, setExportProgress, selectedIndex,
  ]);

  return (
    <div className="flex flex-col h-full gap-4">
      <h2 className="text-sm font-semibold text-gray-500 uppercase tracking-wide">
        Stamp
      </h2>

      {/* Type toggle */}
      <div className="flex rounded-md overflow-hidden border border-gray-200">
        {(['image', 'text'] as StampType[]).map((t) => (
          <button
            key={t}
            onClick={() => setType(t)}
            className={`flex-1 py-1.5 text-sm font-medium transition-colors ${
              stampType === t
                ? 'bg-blue-500 text-white'
                : 'bg-white text-gray-600 hover:bg-gray-50'
            }`}
          >
            {t.charAt(0).toUpperCase() + t.slice(1)}
          </button>
        ))}
      </div>

      {/* Image stamp config */}
      {stampType === 'image' && (
        <div className="space-y-2">
          <button
            onClick={handleImageUpload}
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm text-gray-600 hover:bg-gray-50 transition-colors"
          >
            {imagePreviewUrl ? 'Change Image' : 'Upload Image'}
          </button>
          {imagePreviewUrl && (
            <div className="relative">
              <img
                src={imagePreviewUrl}
                alt="stamp preview"
                className="w-full rounded-md border border-gray-200"
              />
              <button
                onClick={clearImage}
                className="absolute top-1 right-1 rounded-full bg-red-500 text-white w-5 h-5 text-xs flex items-center justify-center"
              >
                &times;
              </button>
            </div>
          )}
        </div>
      )}

      {/* Text stamp config */}
      {stampType === 'text' && (
        <div className="space-y-2">
          <input
            type="text"
            value={text}
            onChange={(e) => setText(e.target.value)}
            placeholder="Stamp text"
            className="w-full rounded-md border border-gray-300 px-3 py-1.5 text-sm"
          />
          <div className="flex gap-2">
            <label className="flex-1">
              <span className="text-xs text-gray-500">Size</span>
              <input
                type="number"
                value={fontSize}
                onChange={(e) => setFontSize(Number(e.target.value))}
                min={6}
                max={200}
                className="w-full rounded-md border border-gray-300 px-2 py-1 text-sm"
              />
            </label>
            <label className="flex-1">
              <span className="text-xs text-gray-500">Color</span>
              <input
                type="color"
                value={color}
                onChange={(e) => setColor(e.target.value)}
                className="w-full h-8 rounded-md border border-gray-300 cursor-pointer"
              />
            </label>
          </div>
        </div>
      )}

      {/* Stamp size controls */}
      {stampType === 'image' && imagePath !== null && (
        <div className="space-y-1">
          <span className="text-xs text-gray-500">Scale (%)</span>
          <input
            type="number"
            value={scalePercent}
            onChange={(e) => setScalePercent(Math.max(1, Number(e.target.value)))}
            min={1}
            className="w-full rounded-md border border-gray-300 px-2 py-1 text-sm"
          />
          <span className="text-xs text-gray-400">
            {widthPt.toFixed(0)} × {heightPt.toFixed(0)} pt
          </span>
        </div>
      )}

      {stampType === 'text' && (
        <div className="space-y-1">
          <span className="text-xs text-gray-500">Stamp Size (points)</span>
          <div className="flex gap-2">
            <input
              type="number"
              value={widthPt}
              onChange={(e) => setSize(Number(e.target.value), heightPt)}
              min={10}
              className="w-20 rounded-md border border-gray-300 px-2 py-1 text-sm"
              placeholder="W"
            />
            <input
              type="number"
              value={heightPt}
              onChange={(e) => setSize(widthPt, Number(e.target.value))}
              min={10}
              className="w-20 rounded-md border border-gray-300 px-2 py-1 text-sm"
              placeholder="H"
            />
          </div>
        </div>
      )}

      {/* Spacer */}
      <div className="flex-1" />

      {/* Apply button */}
      <button
        onClick={handleApplyAll}
        disabled={files.length === 0 || !isPlaced || isExporting}
        className="w-full rounded-md bg-green-500 px-3 py-2 text-sm font-medium text-white hover:bg-green-600 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
      >
        {isExporting
          ? `Exporting ${exportProgress.current}/${exportProgress.total}...`
          : `Apply to All (${files.length})`}
      </button>
    </div>
  );
}
