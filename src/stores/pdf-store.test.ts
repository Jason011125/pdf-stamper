import { describe, it, expect, beforeEach, vi } from 'vitest';
import { usePdfStore, type PdfFile } from './pdf-store';
import { useStampStore, DEFAULT_STAMP_CONFIG } from './stamp-store';

function makeFile(id: string, previewUrl: string | null = null): PdfFile {
  return {
    path: id,
    filename: `${id}.pdf`,
    widthPt: 612,
    heightPt: 792,
    previewUrl,
  };
}

function loadFiles(files: PdfFile[], selectedIndex = 0): void {
  usePdfStore.setState({ files, selectedIndex });
}

function reset(): void {
  usePdfStore.setState({ files: [], selectedIndex: 0 });
  useStampStore.setState({
    defaultConfig: { ...DEFAULT_STAMP_CONFIG },
    overrides: {},
  });
}

describe('pdf-store removeFile (US-FIX2)', () => {
  beforeEach(reset);

  // ── Happy-path index scenarios ────────────────────────────────────────────

  it('remove-first: drops files[0] and the rest shift down', () => {
    loadFiles([makeFile('a'), makeFile('b'), makeFile('c')], 0);
    usePdfStore.getState().removeFile('a');

    expect(usePdfStore.getState().files.map((f) => f.path)).toEqual(['b', 'c']);
  });

  it('remove-middle: drops the middle file', () => {
    loadFiles([makeFile('a'), makeFile('b'), makeFile('c')], 0);
    usePdfStore.getState().removeFile('b');

    expect(usePdfStore.getState().files.map((f) => f.path)).toEqual(['a', 'c']);
  });

  it('remove-last: drops files[N-1]', () => {
    loadFiles([makeFile('a'), makeFile('b'), makeFile('c')], 0);
    usePdfStore.getState().removeFile('c');

    expect(usePdfStore.getState().files.map((f) => f.path)).toEqual(['a', 'b']);
  });

  it('remove-only-one: list becomes empty, selectedIndex = 0', () => {
    loadFiles([makeFile('a')], 0);
    usePdfStore.getState().removeFile('a');

    expect(usePdfStore.getState().files).toEqual([]);
    expect(usePdfStore.getState().selectedIndex).toBe(0);
  });

  it('remove of an unknown pdfId is a no-op', () => {
    loadFiles([makeFile('a'), makeFile('b')], 1);
    const before = usePdfStore.getState();
    usePdfStore.getState().removeFile('not-loaded');

    const after = usePdfStore.getState();
    expect(after.files).toBe(before.files);
    expect(after.selectedIndex).toBe(before.selectedIndex);
  });

  // ── Selection-after-removal ───────────────────────────────────────────────

  it('remove-currently-selected: jumps to first remaining (index 0)', () => {
    loadFiles([makeFile('a'), makeFile('b'), makeFile('c')], 1);
    usePdfStore.getState().removeFile('b');

    expect(usePdfStore.getState().files.map((f) => f.path)).toEqual(['a', 'c']);
    expect(usePdfStore.getState().selectedIndex).toBe(0);
  });

  it('remove-currently-selected (master): jumps to first remaining (now master)', () => {
    loadFiles([makeFile('a'), makeFile('b'), makeFile('c')], 0);
    usePdfStore.getState().removeFile('a');

    expect(usePdfStore.getState().files.map((f) => f.path)).toEqual(['b', 'c']);
    expect(usePdfStore.getState().selectedIndex).toBe(0);
  });

  it('remove-before-selected: tracks the same selected file (index decrements)', () => {
    loadFiles([makeFile('a'), makeFile('b'), makeFile('c')], 2);
    // viewing 'c' (index 2). Remove 'a' (index 0). 'c' is now at index 1.
    usePdfStore.getState().removeFile('a');

    expect(usePdfStore.getState().files.map((f) => f.path)).toEqual(['b', 'c']);
    expect(usePdfStore.getState().selectedIndex).toBe(1);
    expect(
      usePdfStore.getState().files[usePdfStore.getState().selectedIndex]?.path,
    ).toBe('c');
  });

  it('remove-after-selected: keeps selection in place', () => {
    loadFiles([makeFile('a'), makeFile('b'), makeFile('c')], 0);
    // viewing 'a' (index 0). Remove 'c' (index 2). 'a' stays at index 0.
    usePdfStore.getState().removeFile('c');

    expect(usePdfStore.getState().files.map((f) => f.path)).toEqual(['a', 'b']);
    expect(usePdfStore.getState().selectedIndex).toBe(0);
  });

  // ── Side effect: revoke previewUrl ────────────────────────────────────────

  it('revokes the removed file’s previewUrl', () => {
    const revoke = vi.fn();
    URL.revokeObjectURL = revoke;
    loadFiles(
      [makeFile('a', 'blob:fake/a'), makeFile('b', 'blob:fake/b')],
      0,
    );
    usePdfStore.getState().removeFile('a');
    expect(revoke).toHaveBeenCalledWith('blob:fake/a');
  });

  it('does not revoke when the file has no previewUrl yet', () => {
    const revoke = vi.fn();
    URL.revokeObjectURL = revoke;
    loadFiles([makeFile('a', null)], 0);
    usePdfStore.getState().removeFile('a');
    expect(revoke).not.toHaveBeenCalled();
  });
});

// The cross-store integration that the FileList component performs:
// remove the file AND clear the stamp-store override for that PDF.
describe('FileList removal flow — clears stamp-store override', () => {
  beforeEach(() => {
    reset();
    URL.revokeObjectURL = vi.fn();
  });

  function removeAndClear(pdfId: string): void {
    usePdfStore.getState().removeFile(pdfId);
    useStampStore.getState().clearOverride(pdfId);
  }

  it('remove-with-override: override entry is gone from stamp-store', () => {
    loadFiles([makeFile('a'), makeFile('b'), makeFile('c')], 0);
    useStampStore.getState().setOverride('b', { xPt: 200, yPt: 150 });
    expect(useStampStore.getState().overrides['b']).toBeDefined();

    removeAndClear('b');

    expect(usePdfStore.getState().files.map((f) => f.path)).toEqual(['a', 'c']);
    expect(useStampStore.getState().overrides['b']).toBeUndefined();
  });

  it('removing one PDF leaves other overrides intact', () => {
    loadFiles([makeFile('a'), makeFile('b'), makeFile('c')], 0);
    useStampStore.getState().setOverride('b', { xPt: 200 });
    useStampStore.getState().setOverride('c', { xPt: 300 });

    removeAndClear('b');

    expect(useStampStore.getState().overrides).toEqual({ c: { xPt: 300 } });
  });

  it('sequential removals: 5 PDFs, two with overrides, all cleaned up', () => {
    loadFiles(
      [
        makeFile('a'),
        makeFile('b'),
        makeFile('c'),
        makeFile('d'),
        makeFile('e'),
      ],
      0,
    );
    useStampStore.getState().setOverride('b', { xPt: 200 });
    useStampStore.getState().setOverride('d', { xPt: 400 });

    removeAndClear('e');
    removeAndClear('d');
    removeAndClear('c');
    removeAndClear('b');
    removeAndClear('a');

    expect(usePdfStore.getState().files).toEqual([]);
    expect(useStampStore.getState().overrides).toEqual({});
  });
});
