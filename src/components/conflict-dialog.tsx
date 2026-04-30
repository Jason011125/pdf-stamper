import { useEffect, useReducer } from 'react';
import type { Conflict, ConflictDecision } from '../services/pdf-bridge';

export type ConflictAction =
  | { type: 'overwrite' }
  | { type: 'skip' }
  | { type: 'overwrite-all' }
  | { type: 'cancel-all' };

export interface ConflictState {
  total: number;
  currentIndex: number;
  decisions: ConflictDecision[];
  applyAllMode: 'overwrite' | 'skip' | null;
}

export function initialConflictState(total: number): ConflictState {
  return { total, currentIndex: 0, decisions: [], applyAllMode: null };
}

export function conflictReducer(
  state: ConflictState,
  action: ConflictAction,
): ConflictState {
  if (state.currentIndex >= state.total) return state;
  switch (action.type) {
    case 'overwrite': {
      const decisions = [...state.decisions];
      decisions[state.currentIndex] = 'overwrite';
      return { ...state, decisions, currentIndex: state.currentIndex + 1 };
    }
    case 'skip': {
      const decisions = [...state.decisions];
      decisions[state.currentIndex] = 'skip';
      return { ...state, decisions, currentIndex: state.currentIndex + 1 };
    }
    case 'overwrite-all': {
      const decisions = [...state.decisions];
      for (let i = state.currentIndex; i < state.total; i += 1) {
        decisions[i] = 'overwrite';
      }
      return {
        ...state,
        decisions,
        currentIndex: state.total,
        applyAllMode: 'overwrite',
      };
    }
    case 'cancel-all': {
      const decisions = [...state.decisions];
      for (let i = state.currentIndex; i < state.total; i += 1) {
        decisions[i] = 'skip';
      }
      return {
        ...state,
        decisions,
        currentIndex: state.total,
        applyAllMode: 'skip',
      };
    }
  }
}

export function isConflictComplete(state: ConflictState): boolean {
  return state.currentIndex >= state.total;
}

function basename(p: string): string {
  const idx = Math.max(p.lastIndexOf('/'), p.lastIndexOf('\\'));
  return idx >= 0 ? p.slice(idx + 1) : p;
}

interface ConflictDialogProps {
  conflicts: Conflict[];
  onDone: (decisions: ConflictDecision[]) => void;
}

export function ConflictDialog({
  conflicts,
  onDone,
}: ConflictDialogProps): React.JSX.Element | null {
  const [state, dispatch] = useReducer(
    conflictReducer,
    conflicts.length,
    initialConflictState,
  );

  useEffect(() => {
    if (isConflictComplete(state)) {
      onDone(state.decisions);
    }
    // onDone is stable per parent render; we only react to state changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state]);

  if (conflicts.length === 0) return null;
  if (isConflictComplete(state)) return null;

  const current = conflicts[state.currentIndex];
  if (!current) return null;

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Output conflict"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/40"
    >
      <div className="bg-white rounded-lg shadow-xl w-full max-w-md p-5 space-y-4">
        <div className="space-y-1">
          <h3 className="text-base font-semibold text-gray-800">
            File already exists
          </h3>
          <p className="text-xs text-gray-500">
            Conflict {state.currentIndex + 1} of {state.total}
          </p>
        </div>

        <div className="space-y-2 text-sm">
          <div>
            <div className="text-xs text-gray-500">Filename</div>
            <div className="font-medium text-gray-800 break-all">
              {basename(current.output_path)}
            </div>
          </div>
          <div>
            <div className="text-xs text-gray-500">Output path</div>
            <div className="text-gray-700 break-all">{current.output_path}</div>
          </div>
        </div>

        <div className="grid grid-cols-2 gap-2 pt-2">
          <button
            onClick={() => dispatch({ type: 'overwrite' })}
            className="rounded-md bg-blue-500 hover:bg-blue-600 text-white text-sm font-medium py-2"
          >
            Overwrite
          </button>
          <button
            onClick={() => dispatch({ type: 'skip' })}
            className="rounded-md bg-gray-200 hover:bg-gray-300 text-gray-800 text-sm font-medium py-2"
          >
            Cancel
          </button>
          <button
            onClick={() => dispatch({ type: 'overwrite-all' })}
            className="rounded-md bg-blue-100 hover:bg-blue-200 text-blue-700 text-sm font-medium py-2"
          >
            Overwrite all
          </button>
          <button
            onClick={() => dispatch({ type: 'cancel-all' })}
            className="rounded-md bg-gray-100 hover:bg-gray-200 text-gray-700 text-sm font-medium py-2"
          >
            Cancel all
          </button>
        </div>
      </div>
    </div>
  );
}
