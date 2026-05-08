import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/react';
import {
  ConflictDialog,
  conflictReducer,
  initialConflictState,
  isConflictComplete,
  type ConflictAction,
  type ConflictState,
} from './conflict-dialog';
import type { Conflict } from '../services/pdf-bridge';
import { open } from '@tauri-apps/plugin-dialog';

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
}));

const openMock = vi.mocked(open);

function run(initialTotal: number, actions: ConflictAction[]): ConflictState {
  return actions.reduce(
    (state, action) => conflictReducer(state, action),
    initialConflictState(initialTotal),
  );
}

describe('conflictReducer', () => {
  it('starts with no decisions, currentIndex 0, applyAllMode null', () => {
    const s = initialConflictState(5);
    expect(s).toEqual({
      total: 5,
      currentIndex: 0,
      decisions: [],
      applyAllMode: null,
    });
    expect(isConflictComplete(s)).toBe(false);
  });

  it('"overwrite" records overwrite for currentIndex and advances', () => {
    const s = run(3, [{ type: 'overwrite' }]);
    expect(s.decisions).toEqual(['overwrite']);
    expect(s.currentIndex).toBe(1);
    expect(s.applyAllMode).toBeNull();
    expect(isConflictComplete(s)).toBe(false);
  });

  it('"skip" records skip for currentIndex and advances', () => {
    const s = run(3, [{ type: 'skip' }]);
    expect(s.decisions).toEqual(['skip']);
    expect(s.currentIndex).toBe(1);
    expect(s.applyAllMode).toBeNull();
  });

  it('overwrite all individually completes with all overwrites', () => {
    const s = run(3, [
      { type: 'overwrite' },
      { type: 'overwrite' },
      { type: 'overwrite' },
    ]);
    expect(s.decisions).toEqual(['overwrite', 'overwrite', 'overwrite']);
    expect(isConflictComplete(s)).toBe(true);
  });

  it('"overwrite-all" fills the rest with overwrite and completes', () => {
    const s = run(5, [{ type: 'overwrite-all' }]);
    expect(s.decisions).toEqual([
      'overwrite',
      'overwrite',
      'overwrite',
      'overwrite',
      'overwrite',
    ]);
    expect(s.currentIndex).toBe(5);
    expect(s.applyAllMode).toBe('overwrite');
    expect(isConflictComplete(s)).toBe(true);
  });

  it('"cancel-all" fills the rest with skip and completes', () => {
    const s = run(5, [{ type: 'cancel-all' }]);
    expect(s.decisions).toEqual(['skip', 'skip', 'skip', 'skip', 'skip']);
    expect(s.currentIndex).toBe(5);
    expect(s.applyAllMode).toBe('skip');
    expect(isConflictComplete(s)).toBe(true);
  });

  // The acceptance criterion's headline scenario
  it('"overwrite, cancel-all on 5 conflicts" → [overwrite, skip, skip, skip, skip]', () => {
    const s = run(5, [{ type: 'overwrite' }, { type: 'cancel-all' }]);
    expect(s.decisions).toEqual([
      'overwrite',
      'skip',
      'skip',
      'skip',
      'skip',
    ]);
    expect(s.applyAllMode).toBe('skip');
    expect(isConflictComplete(s)).toBe(true);
  });

  it('"skip, overwrite-all on 4 conflicts" → [skip, overwrite, overwrite, overwrite]', () => {
    const s = run(4, [{ type: 'skip' }, { type: 'overwrite-all' }]);
    expect(s.decisions).toEqual([
      'skip',
      'overwrite',
      'overwrite',
      'overwrite',
    ]);
    expect(s.applyAllMode).toBe('overwrite');
    expect(isConflictComplete(s)).toBe(true);
  });

  it('mixed "overwrite, skip, overwrite, skip" on 4 conflicts', () => {
    const s = run(4, [
      { type: 'overwrite' },
      { type: 'skip' },
      { type: 'overwrite' },
      { type: 'skip' },
    ]);
    expect(s.decisions).toEqual(['overwrite', 'skip', 'overwrite', 'skip']);
    expect(s.applyAllMode).toBeNull();
    expect(isConflictComplete(s)).toBe(true);
  });

  it('actions after completion are no-ops', () => {
    const completed = run(2, [{ type: 'overwrite' }, { type: 'skip' }]);
    const after = conflictReducer(completed, { type: 'overwrite' });
    expect(after).toBe(completed);
  });

  it('zero-conflict state is already complete', () => {
    expect(isConflictComplete(initialConflictState(0))).toBe(true);
  });
});

// US-FIX3: clicking "Overwrite all" / "Cancel all" inside the dialog must NEVER
// trigger a Tauri open() / file picker. The dialog only dispatches reducer
// actions; the parent's onDone handler runs the export with no further dialogs.
describe('ConflictDialog component (US-FIX3) — no file picker on apply-all', () => {
  beforeEach(() => {
    openMock.mockReset();
    cleanup();
  });

  function makeConflicts(n: number): Conflict[] {
    return Array.from({ length: n }, (_, i) => ({
      idx: i,
      output_path: `/Users/x/Downloads/file${i}.pdf`,
    }));
  }

  it('Overwrite all: onDone receives 5 overwrites and Tauri open() is not invoked', () => {
    const onDone = vi.fn();
    render(
      <ConflictDialog conflicts={makeConflicts(5)} onDone={onDone} />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Overwrite all' }));

    expect(onDone).toHaveBeenCalledTimes(1);
    expect(onDone).toHaveBeenCalledWith([
      'overwrite',
      'overwrite',
      'overwrite',
      'overwrite',
      'overwrite',
    ]);
    expect(openMock).not.toHaveBeenCalled();
  });

  it('Cancel all: onDone receives 5 skips and Tauri open() is not invoked', () => {
    const onDone = vi.fn();
    render(
      <ConflictDialog conflicts={makeConflicts(5)} onDone={onDone} />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Cancel all' }));

    expect(onDone).toHaveBeenCalledTimes(1);
    expect(onDone).toHaveBeenCalledWith([
      'skip',
      'skip',
      'skip',
      'skip',
      'skip',
    ]);
    expect(openMock).not.toHaveBeenCalled();
  });

  it('Overwrite (single) advances to next conflict and does not invoke open()', () => {
    const onDone = vi.fn();
    render(
      <ConflictDialog conflicts={makeConflicts(3)} onDone={onDone} />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Overwrite' }));

    expect(onDone).not.toHaveBeenCalled();
    expect(openMock).not.toHaveBeenCalled();
    // Conflict 2 of 3 is now displayed.
    expect(screen.getByText('Conflict 2 of 3')).toBeInTheDocument();
  });

  it('Cancel (single) advances to next conflict and does not invoke open()', () => {
    const onDone = vi.fn();
    render(
      <ConflictDialog conflicts={makeConflicts(3)} onDone={onDone} />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));

    expect(onDone).not.toHaveBeenCalled();
    expect(openMock).not.toHaveBeenCalled();
    expect(screen.getByText('Conflict 2 of 3')).toBeInTheDocument();
  });
});
