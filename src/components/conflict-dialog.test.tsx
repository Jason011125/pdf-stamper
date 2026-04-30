import { describe, it, expect } from 'vitest';
import {
  conflictReducer,
  initialConflictState,
  isConflictComplete,
  type ConflictAction,
  type ConflictState,
} from './conflict-dialog';

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
