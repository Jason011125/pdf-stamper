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

export interface StampPosition {
  x: number;
  y: number;
}

export interface StampParams {
  paths: string[];
  stampType: 'image' | 'text';
  imagePath: string | null;
  text: string | null;
  fontSize: number | null;
  fontName: string | null;
  color: string | null;
  /** Per-file stamp positions; must be same length as paths */
  positions: StampPosition[];
  width: number;
  height: number;
  /** Rotation in degrees, clockwise on screen (0–359) */
  rotation: number;
  outputDir: string;
}

export async function stampAllPdfs(params: StampParams): Promise<string[]> {
  return invoke<string[]>('stamp_pdfs', {
    paths: params.paths,
    stampType: params.stampType,
    imagePath: params.imagePath,
    text: params.text,
    fontSize: params.fontSize,
    fontName: params.fontName,
    color: params.color,
    positions: params.positions,
    width: params.width,
    height: params.height,
    rotation: params.rotation,
    outputDir: params.outputDir,
  });
}
