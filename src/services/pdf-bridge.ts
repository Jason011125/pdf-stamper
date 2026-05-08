import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';

export interface PdfInfo {
  path: string;
  filename: string;
  width_pt: number;
  height_pt: number;
}

export async function openPdfDialog(): Promise<string[] | null> {
  const selected = await open({
    multiple: true,
    filters: [{ name: 'PDF Files', extensions: ['pdf'] }],
  });
  if (!selected) return null;
  return Array.isArray(selected) ? selected : [selected];
}

export async function loadPdfs(paths: string[]): Promise<PdfInfo[]> {
  return invoke<PdfInfo[]>('open_pdfs', { paths });
}

export async function renderPage(
  path: string,
  width: number,
): Promise<string> {
  const bytes = await invoke<number[]>('render_page', { path, width });
  const uint8 = new Uint8Array(bytes);
  const blob = new Blob([uint8], { type: 'image/png' });
  return URL.createObjectURL(blob);
}

export async function selectOutputDir(): Promise<string | null> {
  const selected = await open({ directory: true });
  if (!selected) return null;
  return typeof selected === 'string' ? selected : null;
}

export interface ConflictInput {
  path: string;
}

export interface Conflict {
  idx: number;
  output_path: string;
}

export type ConflictDecision = 'overwrite' | 'skip';

export async function checkOutputConflicts(
  inputs: ConflictInput[],
  outputDir: string,
): Promise<Conflict[]> {
  return invoke<Conflict[]>('check_output_conflicts', {
    inputs,
    outputDir,
  });
}

/** One PDF's complete stamp configuration. The frontend builds an array of
 * these (one per loaded PDF, computed via `getEffectiveConfig(pdfId)`) before
 * invoking `stampAllPdfs`. Stamp content fields (imagePath/text/...) are
 * per-job for symmetry with the store's `StampConfig`, even though the UI
 * keeps stamp content global today. */
export interface StampJob {
  path: string;
  stampType: 'image' | 'text';
  imagePath: string | null;
  text: string | null;
  fontSize: number | null;
  fontName: string | null;
  color: string | null;
  x: number;
  y: number;
  width: number;
  height: number;
  /** Stamp rotation in degrees CCW around the stamp's center. */
  rotationDeg?: number;
}

export interface StampParams {
  jobs: StampJob[];
  outputDir: string;
  skipIndices?: number[];
  /** When true, the Rust side rasterizes each stamped page at high DPI and
   * writes a fresh single-page PDF whose only content is that JPEG. The
   * stamp ends up baked into pixels so editors like WPS PDF Edit can no
   * longer manipulate it. */
  flatten: boolean;
}

export async function stampAllPdfs(params: StampParams): Promise<string[]> {
  return invoke<string[]>('stamp_pdfs', {
    jobs: params.jobs,
    outputDir: params.outputDir,
    skipIndices: params.skipIndices,
    flatten: params.flatten,
  });
}
