import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
}));

import { invoke } from '@tauri-apps/api/core';
import {
  checkOutputConflicts,
  stampAllPdfs,
  type Conflict,
  type ConflictDecision,
} from './pdf-bridge';

const invokeMock = vi.mocked(invoke);

describe('checkOutputConflicts', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it('forwards inputs/outputDir/suffix to the Rust command with camelCase keys', async () => {
    invokeMock.mockResolvedValueOnce([]);
    await checkOutputConflicts(
      [{ path: '/a.pdf' }, { path: '/b.pdf' }],
      '/out',
      '-stamped',
    );
    expect(invokeMock).toHaveBeenCalledWith('check_output_conflicts', {
      inputs: [{ path: '/a.pdf' }, { path: '/b.pdf' }],
      outputDir: '/out',
      suffix: '-stamped',
    });
  });

  it('returns the parsed Conflict[] response shape (snake_case output_path)', async () => {
    const fakeResponse: Conflict[] = [
      { idx: 0, output_path: '/out/a-stamped.pdf' },
      { idx: 2, output_path: '/out/c-stamped.pdf' },
    ];
    invokeMock.mockResolvedValueOnce(fakeResponse);

    const result = await checkOutputConflicts(
      [{ path: '/a.pdf' }, { path: '/b.pdf' }, { path: '/c.pdf' }],
      '/out',
      '-stamped',
    );

    expect(result).toEqual(fakeResponse);
    expect(result[0]?.idx).toBe(0);
    expect(result[0]?.output_path).toBe('/out/a-stamped.pdf');
    expect(result[1]?.idx).toBe(2);
  });
});

describe('stampAllPdfs skipIndices', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  const baseParams = {
    paths: ['/a.pdf', '/b.pdf'],
    stampType: 'text' as const,
    imagePath: null,
    text: 'STAMP',
    fontSize: 24,
    fontName: 'Helvetica',
    color: '#000000',
    x: 10,
    y: 20,
    width: 100,
    height: 50,
    outputDir: '/out',
  };

  it('forwards skipIndices to the stamp_pdfs command when provided', async () => {
    invokeMock.mockResolvedValueOnce([]);
    await stampAllPdfs({ ...baseParams, skipIndices: [1] });
    expect(invokeMock).toHaveBeenCalledWith(
      'stamp_pdfs',
      expect.objectContaining({ skipIndices: [1] }),
    );
  });

  it('passes skipIndices as undefined when omitted (Rust treats as None)', async () => {
    invokeMock.mockResolvedValueOnce([]);
    await stampAllPdfs(baseParams);
    const call = invokeMock.mock.calls[0];
    expect(call?.[0]).toBe('stamp_pdfs');
    const args = call?.[1] as Record<string, unknown> | undefined;
    expect(args?.skipIndices).toBeUndefined();
  });
});

describe('ConflictDecision type', () => {
  it('accepts the two valid string literals', () => {
    const decisions: ConflictDecision[] = ['overwrite', 'skip'];
    expect(decisions).toEqual(['overwrite', 'skip']);
  });
});
