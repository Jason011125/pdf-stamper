import { useCallback, useState } from 'react';
import { useStampStore, type StampType } from '../stores/stamp-store';
import { usePdfStore } from '../stores/pdf-store';
import {
  stampAllPdfs,
  selectOutputDir,
  checkOutputConflicts,
  type Conflict,
  type ConflictDecision,
  type StampJob,
} from '../services/pdf-bridge';
import { open } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';
import { ConflictDialog } from './conflict-dialog';

const STAMP_SUFFIX = '-stamped';

async function readImagePreview(path: string): Promise<string> {
  const bytes = await invoke<number[]>('read_file_bytes', { path });
  const uint8 = new Uint8Array(bytes);
  const blob = new Blob([uint8], { type: 'image/png' });
  return URL.createObjectURL(blob);
}

async function readImageDimensions(url: string): Promise<{ w: number; h: number }> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve({ w: img.naturalWidth, h: img.naturalHeight });
    img.onerror = reject;
    img.src = url;
  });
}

const DEFAULT_STAMP_LONG_SIDE = 100;

export function StampControls(): React.JSX.Element {
  const files = usePdfStore((s) => s.files);
  const selectedIndex = usePdfStore((s) => s.selectedIndex);
  const currentPdfId = files[selectedIndex]?.path ?? '';

  // Reads go through getEffectiveConfig(currentPdfId): merged default + override.
  // Writes for position/size/rotation route through applyEdit (first call writes
  // default as bootstrap, subsequent calls write overrides[currentPdfId]).
  // Image / text / color / font / lockAspect remain on default via the existing
  // shims — those are global stamp setup, not per-PDF position/size/rotation.
  const stampType = useStampStore((s) => s.getEffectiveConfig(currentPdfId).type);
  const setType = useStampStore((s) => s.setType);
  const imagePreviewUrl = useStampStore(
    (s) => s.getEffectiveConfig(currentPdfId).imagePreviewUrl,
  );
  const setImage = useStampStore((s) => s.setImage);
  const clearImage = useStampStore((s) => s.clearImage);
  const text = useStampStore((s) => s.getEffectiveConfig(currentPdfId).text);
  const setText = useStampStore((s) => s.setText);
  const fontSize = useStampStore((s) => s.getEffectiveConfig(currentPdfId).fontSize);
  const setFontSize = useStampStore((s) => s.setFontSize);
  const color = useStampStore((s) => s.getEffectiveConfig(currentPdfId).color);
  const setColor = useStampStore((s) => s.setColor);
  const widthPt = useStampStore((s) => s.getEffectiveConfig(currentPdfId).widthPt);
  const heightPt = useStampStore((s) => s.getEffectiveConfig(currentPdfId).heightPt);
  const setSize = useStampStore((s) => s.setSize);
  const lockAspect = useStampStore(
    (s) => s.getEffectiveConfig(currentPdfId).lockAspect,
  );
  const setLockAspect = useStampStore((s) => s.setLockAspect);
  const lockedAspect = useStampStore(
    (s) => s.getEffectiveConfig(currentPdfId).lockedAspect,
  );
  const setLockedAspect = useStampStore((s) => s.setLockedAspect);
  const xPt = useStampStore((s) => s.getEffectiveConfig(currentPdfId).xPt);
  const yPt = useStampStore((s) => s.getEffectiveConfig(currentPdfId).yPt);
  const rotationDeg = useStampStore(
    (s) => s.getEffectiveConfig(currentPdfId).rotationDeg,
  );
  const applyEdit = useStampStore((s) => s.applyEdit);
  const isPlaced = useStampStore((s) => s.isPlaced);
  const isExporting = useStampStore((s) => s.isExporting);
  const setExporting = useStampStore((s) => s.setExporting);
  const exportProgress = useStampStore((s) => s.exportProgress);
  const setExportProgress = useStampStore((s) => s.setExportProgress);

  const handleImageUpload = useCallback(async () => {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp'] }],
    });
    if (!selected) return;

    const path = typeof selected === 'string' ? selected : selected;
    const previewUrl = await readImagePreview(path);

    // Snap stamp box to the image's natural aspect so the preview's blue box
    // matches what gets exported. Long side defaults to DEFAULT_STAMP_LONG_SIDE.
    // The aspect is captured into the store so subsequent edits in lock mode
    // survive any intermediate 0/NaN typed into the W or H field.
    try {
      const { w: natW, h: natH } = await readImageDimensions(previewUrl);
      if (natW > 0 && natH > 0) {
        const aspect = natW / natH;
        const longSide = DEFAULT_STAMP_LONG_SIDE;
        const newW = aspect >= 1 ? longSide : longSide * aspect;
        const newH = aspect >= 1 ? longSide / aspect : longSide;
        setLockedAspect(aspect);
        setSize(newW, newH);
      }
    } catch {
      // Fall through with whatever size is already set if the image fails to load
      // for dimension probing — the user can still edit W/H manually.
    }

    setImage(path, previewUrl);
  }, [setImage, setSize, setLockedAspect]);

  const handleWidthChange = useCallback(
    (newW: number) => {
      if (!Number.isFinite(newW) || newW <= 0) {
        // Allow the input to display 0/empty during typing, but don't propagate
        // a degenerate value into store — keeps locked aspect intact.
        applyEdit(currentPdfId, { widthPt: Math.max(0, newW), heightPt });
        return;
      }
      if (lockAspect && lockedAspect > 0) {
        applyEdit(currentPdfId, { widthPt: newW, heightPt: newW / lockedAspect });
      } else {
        applyEdit(currentPdfId, { widthPt: newW, heightPt });
      }
    },
    [lockAspect, lockedAspect, heightPt, applyEdit, currentPdfId],
  );

  const handleHeightChange = useCallback(
    (newH: number) => {
      if (!Number.isFinite(newH) || newH <= 0) {
        applyEdit(currentPdfId, { widthPt, heightPt: Math.max(0, newH) });
        return;
      }
      if (lockAspect && lockedAspect > 0) {
        applyEdit(currentPdfId, { widthPt: newH * lockedAspect, heightPt: newH });
      } else {
        applyEdit(currentPdfId, { widthPt, heightPt: newH });
      }
    },
    [lockAspect, lockedAspect, widthPt, applyEdit, currentPdfId],
  );

  // When the user toggles lock back ON, capture the *current* W/H ratio so
  // they can lock to whatever (possibly distorted) shape they just made.
  // Falls back to the existing lockedAspect if W or H is currently 0.
  const handleLockChange = useCallback(
    (locked: boolean) => {
      if (locked && widthPt > 0 && heightPt > 0) {
        setLockedAspect(widthPt / heightPt);
      }
      setLockAspect(locked);
    },
    [widthPt, heightPt, setLockAspect, setLockedAspect],
  );

  const [pendingConflicts, setPendingConflicts] = useState<{
    conflicts: Conflict[];
    outputDir: string;
  } | null>(null);

  // getEffectiveConfig is a function selector (returns a fresh object on each
  // call). Subscribing to it directly would re-render every state change —
  // instead we pull it from the store imperatively at submit time.
  const getEffectiveConfig = useStampStore((s) => s.getEffectiveConfig);

  const runStamp = useCallback(
    async (outputDir: string, skipIndices: number[]) => {
      setExporting(true);
      setExportProgress(0, files.length);
      try {
        // Assemble per-PDF jobs: each loaded PDF gets its effective config
        // (default merged with that PDF's override). Position/size/rotation
        // can therefore differ per file; stamp content is the same across
        // files in today's UI but the per-job shape leaves room for that.
        const jobs: StampJob[] = files.map((f) => {
          const cfg = getEffectiveConfig(f.path);
          return {
            path: f.path,
            stampType: cfg.type,
            imagePath: cfg.type === 'image' ? cfg.imagePath : null,
            text: cfg.type === 'text' ? cfg.text : null,
            fontSize: cfg.type === 'text' ? cfg.fontSize : null,
            fontName: cfg.type === 'text' ? cfg.fontFamily : null,
            color: cfg.type === 'text' ? cfg.color : null,
            x: cfg.xPt,
            y: cfg.yPt,
            width: cfg.widthPt,
            height: cfg.heightPt,
            rotationDeg: cfg.rotationDeg,
          };
        });
        const result = await stampAllPdfs({
          jobs,
          outputDir,
          skipIndices: skipIndices.length > 0 ? skipIndices : undefined,
        });
        setExportProgress(result.length, files.length);
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        alert(`Export failed: ${message}`);
      } finally {
        setExporting(false);
      }
    },
    [files, getEffectiveConfig, setExporting, setExportProgress],
  );

  const handleApplyAll = useCallback(async () => {
    if (files.length === 0 || !isPlaced) return;

    const dir = await selectOutputDir();
    if (!dir) return;

    const conflicts = await checkOutputConflicts(
      files.map((f) => ({ path: f.path })),
      dir,
      STAMP_SUFFIX,
    );

    if (conflicts.length === 0) {
      await runStamp(dir, []);
      return;
    }

    setPendingConflicts({ conflicts, outputDir: dir });
  }, [files, isPlaced, runStamp]);

  const handleConflictsResolved = useCallback(
    async (decisions: ConflictDecision[]) => {
      const pending = pendingConflicts;
      setPendingConflicts(null);
      if (!pending) return;
      const skipIndices = pending.conflicts
        .filter((_, i) => decisions[i] === 'skip')
        .map((c) => c.idx);
      await runStamp(pending.outputDir, skipIndices);
    },
    [pendingConflicts, runStamp],
  );

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

      {/* Stamp dimensions */}
      <div className="space-y-1">
        <div className="flex items-center justify-between">
          <span className="text-xs text-gray-500">Stamp Size (points)</span>
          <label className="flex items-center gap-1 text-xs text-gray-500 cursor-pointer select-none">
            <input
              type="checkbox"
              checked={lockAspect}
              onChange={(e) => handleLockChange(e.target.checked)}
              className="cursor-pointer"
            />
            Lock aspect
          </label>
        </div>
        <div className="flex items-center gap-2">
          <input
            type="number"
            value={Math.round(widthPt * 10) / 10}
            onChange={(e) => handleWidthChange(Number(e.target.value))}
            min={10}
            className="w-20 rounded-md border border-gray-300 px-2 py-1 text-sm"
            placeholder="W"
          />
          <span className="text-xs text-gray-400">{lockAspect ? '↔' : '✕'}</span>
          <input
            type="number"
            value={Math.round(heightPt * 10) / 10}
            onChange={(e) => handleHeightChange(Number(e.target.value))}
            min={10}
            className="w-20 rounded-md border border-gray-300 px-2 py-1 text-sm"
            placeholder="H"
          />
        </div>
      </div>

      {/* Rotation */}
      <div className="space-y-1">
        <div className="flex items-center justify-between">
          <span className="text-xs text-gray-500">Rotation (deg, CCW)</span>
          <button
            type="button"
            onClick={() => applyEdit(currentPdfId, { rotationDeg: 0 })}
            className="text-xs text-blue-500 hover:underline"
          >
            Reset
          </button>
        </div>
        <div className="flex items-center gap-2">
          <input
            type="range"
            min={0}
            max={360}
            step={1}
            value={rotationDeg}
            onChange={(e) =>
              applyEdit(currentPdfId, { rotationDeg: Number(e.target.value) })
            }
            className="flex-1"
            aria-label="Stamp rotation slider"
          />
          <input
            type="number"
            min={0}
            max={360}
            step={1}
            value={Math.round(rotationDeg)}
            onChange={(e) =>
              applyEdit(currentPdfId, { rotationDeg: Number(e.target.value) })
            }
            className="w-16 rounded-md border border-gray-300 px-2 py-1 text-sm"
            aria-label="Stamp rotation degrees"
          />
        </div>
        <div className="flex gap-2">
          <button
            type="button"
            onClick={() =>
              applyEdit(currentPdfId, {
                rotationDeg: (rotationDeg - 90 + 360) % 360,
              })
            }
            disabled={!currentPdfId}
            aria-label="Rotate stamp 90 degrees clockwise"
            className="flex-1 rounded-md border border-gray-300 px-2 py-1 text-xs text-gray-600 hover:bg-gray-50 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            −90°
          </button>
          <button
            type="button"
            onClick={() =>
              applyEdit(currentPdfId, {
                rotationDeg: (rotationDeg + 90) % 360,
              })
            }
            disabled={!currentPdfId}
            aria-label="Rotate stamp 90 degrees counter-clockwise"
            className="flex-1 rounded-md border border-gray-300 px-2 py-1 text-xs text-gray-600 hover:bg-gray-50 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            +90°
          </button>
        </div>
      </div>

      {/* Position info */}
      {isPlaced && (
        <div className="text-xs text-gray-400">
          Position: ({xPt.toFixed(1)}, {yPt.toFixed(1)}) pt
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

      {pendingConflicts && (
        <ConflictDialog
          conflicts={pendingConflicts.conflicts}
          onDone={handleConflictsResolved}
        />
      )}
    </div>
  );
}
