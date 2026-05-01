import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/react';
import { StampControls } from './stamp-controls';
import {
  useStampStore,
  DEFAULT_STAMP_CONFIG,
} from '../stores/stamp-store';
import { usePdfStore, type PdfFile } from '../stores/pdf-store';

vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

function reset(): void {
  useStampStore.setState({
    defaultConfig: { ...DEFAULT_STAMP_CONFIG },
    overrides: {},
    isPlaced: false,
    isExporting: false,
    exportProgress: { current: 0, total: 0 },
  });
  usePdfStore.setState({ files: [], selectedIndex: 0 });
}

function loadPdfs(ids: string[]): void {
  const files: PdfFile[] = ids.map((id) => ({
    path: id,
    filename: id,
    widthPt: 612,
    heightPt: 792,
    previewUrl: null,
  }));
  usePdfStore.setState({ files, selectedIndex: 0 });
}

function selectByPath(pdfId: string): void {
  const idx = usePdfStore.getState().files.findIndex((f) => f.path === pdfId);
  if (idx >= 0) usePdfStore.setState({ selectedIndex: idx });
}

describe('StampControls quick rotation buttons (US-R3)', () => {
  beforeEach(() => {
    cleanup();
    reset();
  });

  // (a) +90° four times wraps back to original via mod 360.
  it('+90° four times returns rotationDeg to original (mod 360 wrap)', () => {
    loadPdfs(['a', 'b', 'c']);
    render(<StampControls />);

    const plus = screen.getByRole('button', { name: 'Rotate stamp 90 degrees counter-clockwise' });
    fireEvent.click(plus);
    expect(useStampStore.getState().getEffectiveConfig('a').rotationDeg).toBe(90);
    fireEvent.click(plus);
    expect(useStampStore.getState().getEffectiveConfig('a').rotationDeg).toBe(180);
    fireEvent.click(plus);
    expect(useStampStore.getState().getEffectiveConfig('a').rotationDeg).toBe(270);
    fireEvent.click(plus);
    expect(useStampStore.getState().getEffectiveConfig('a').rotationDeg).toBe(0);
  });

  // (d) -90° from 0 yields 270.
  it('−90° from 0 yields 270 (negative wraps via mod 360)', () => {
    loadPdfs(['a']);
    render(<StampControls />);

    fireEvent.click(screen.getByRole('button', { name: 'Rotate stamp 90 degrees clockwise' }));
    expect(useStampStore.getState().getEffectiveConfig('a').rotationDeg).toBe(270);
  });

  // (b) Click on master PDF propagates to all unoverridden PDFs.
  it('+90° on master writes default → propagates to all unoverridden PDFs', () => {
    loadPdfs(['a', 'b', 'c', 'd', 'e']);
    render(<StampControls />);

    fireEvent.click(screen.getByRole('button', { name: 'Rotate stamp 90 degrees counter-clockwise' }));

    expect(useStampStore.getState().overrides).toEqual({});
    for (const id of ['a', 'b', 'c', 'd', 'e']) {
      expect(useStampStore.getState().getEffectiveConfig(id).rotationDeg).toBe(90);
    }
  });

  // (c) Click on non-master PDF only changes that PDF's effective config.
  it('+90° on non-master writes overrides[pdfId] only', () => {
    loadPdfs(['a', 'b', 'c']);
    selectByPath('b');
    render(<StampControls />);

    fireEvent.click(screen.getByRole('button', { name: 'Rotate stamp 90 degrees counter-clockwise' }));

    expect(useStampStore.getState().overrides).toEqual({ b: { rotationDeg: 90 } });
    expect(useStampStore.getState().getEffectiveConfig('a').rotationDeg).toBe(0);
    expect(useStampStore.getState().getEffectiveConfig('b').rotationDeg).toBe(90);
    expect(useStampStore.getState().getEffectiveConfig('c').rotationDeg).toBe(0);
  });

  it('−90° on master writes default and propagates', () => {
    loadPdfs(['a', 'b']);
    render(<StampControls />);

    fireEvent.click(screen.getByRole('button', { name: 'Rotate stamp 90 degrees clockwise' }));

    expect(useStampStore.getState().overrides).toEqual({});
    expect(useStampStore.getState().getEffectiveConfig('a').rotationDeg).toBe(270);
    expect(useStampStore.getState().getEffectiveConfig('b').rotationDeg).toBe(270);
  });

  it('quick-rotate buttons are disabled when no PDF is loaded', () => {
    // No loadPdfs — files[] is empty so currentPdfId === ''.
    render(<StampControls />);

    expect(
      screen.getByRole('button', { name: 'Rotate stamp 90 degrees counter-clockwise' }),
    ).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Rotate stamp 90 degrees clockwise' })).toBeDisabled();
  });

  it('routes through applyEdit (master rule preserved): non-master click does not mutate default', () => {
    loadPdfs(['a', 'b']);
    useStampStore.setState({
      defaultConfig: { ...DEFAULT_STAMP_CONFIG, rotationDeg: 30 },
    });
    selectByPath('b');
    render(<StampControls />);

    fireEvent.click(screen.getByRole('button', { name: 'Rotate stamp 90 degrees counter-clockwise' }));

    // Default stays at 30; override on 'b' is 30 + 90 = 120.
    expect(useStampStore.getState().defaultConfig.rotationDeg).toBe(30);
    expect(useStampStore.getState().getEffectiveConfig('b').rotationDeg).toBe(120);
    expect(useStampStore.getState().getEffectiveConfig('a').rotationDeg).toBe(30);
  });
});
