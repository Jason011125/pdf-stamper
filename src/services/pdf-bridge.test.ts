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

  it('forwards inputs/outputDir to the Rust command with camelCase keys', async () => {
    invokeMock.mockResolvedValueOnce([]);
    await checkOutputConflicts(
      [{ path: '/a.pdf' }, { path: '/b.pdf' }],
      '/out',
    );
    expect(invokeMock).toHaveBeenCalledWith('check_output_conflicts', {
      inputs: [{ path: '/a.pdf' }, { path: '/b.pdf' }],
      outputDir: '/out',
    });
  });

  it('returns the parsed Conflict[] response shape (snake_case output_path)', async () => {
    const fakeResponse: Conflict[] = [
      { idx: 0, output_path: '/out/a.pdf' },
      { idx: 2, output_path: '/out/c.pdf' },
    ];
    invokeMock.mockResolvedValueOnce(fakeResponse);

    const result = await checkOutputConflicts(
      [{ path: '/a.pdf' }, { path: '/b.pdf' }, { path: '/c.pdf' }],
      '/out',
    );

    expect(result).toEqual(fakeResponse);
    expect(result[0]?.idx).toBe(0);
    expect(result[0]?.output_path).toBe('/out/a.pdf');
    expect(result[1]?.idx).toBe(2);
  });
});

describe('stampAllPdfs jobs payload', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  const baseJob = {
    path: '/a.pdf',
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
  };

  const baseParams = {
    jobs: [baseJob, { ...baseJob, path: '/b.pdf' }],
    outputDir: '/out',
  };

  it('forwards jobs and outputDir to stamp_pdfs (camelCase keys)', async () => {
    invokeMock.mockResolvedValueOnce([]);
    await stampAllPdfs(baseParams);
    expect(invokeMock).toHaveBeenCalledWith('stamp_pdfs', {
      jobs: baseParams.jobs,
      outputDir: '/out',
      skipIndices: undefined,
    });
  });

  it('forwards skipIndices when provided', async () => {
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

  it('forwards per-job rotationDeg verbatim inside jobs', async () => {
    invokeMock.mockResolvedValueOnce([]);
    const jobs = [
      { ...baseJob, path: '/a.pdf', rotationDeg: 90 },
      { ...baseJob, path: '/b.pdf', rotationDeg: 180 },
    ];
    await stampAllPdfs({ jobs, outputDir: '/out' });
    const call = invokeMock.mock.calls[0];
    const args = call?.[1] as Record<string, unknown> | undefined;
    const sentJobs = args?.jobs as Array<Record<string, unknown>>;
    expect(sentJobs[0]?.rotationDeg).toBe(90);
    expect(sentJobs[1]?.rotationDeg).toBe(180);
  });

  it('per-PDF jobs may carry distinct (x, y) values', async () => {
    invokeMock.mockResolvedValueOnce([]);
    const jobs = [
      { ...baseJob, path: '/a.pdf', x: 10, y: 20 },
      { ...baseJob, path: '/b.pdf', x: 300, y: 400 },
    ];
    await stampAllPdfs({ jobs, outputDir: '/out' });
    const args = invokeMock.mock.calls[0]?.[1] as Record<string, unknown>;
    const sentJobs = args.jobs as Array<Record<string, unknown>>;
    expect(sentJobs[0]?.x).toBe(10);
    expect(sentJobs[0]?.y).toBe(20);
    expect(sentJobs[1]?.x).toBe(300);
    expect(sentJobs[1]?.y).toBe(400);
  });
});

describe('ConflictDecision type', () => {
  it('accepts the two valid string literals', () => {
    const decisions: ConflictDecision[] = ['overwrite', 'skip'];
    expect(decisions).toEqual(['overwrite', 'skip']);
  });
});
