import { describe, it, expect } from 'vitest';
import { stampRotationStyle } from './preview-pane';

describe('stampRotationStyle', () => {
  it('returns identity rotation at 0', () => {
    // -0 stringifies to "0" in template literals, so the no-op case has no
    // sign in the output.
    expect(stampRotationStyle(0)).toEqual({
      transform: 'rotate(0deg)',
      transformOrigin: 'center',
    });
  });

  // rotationDeg is stored as CCW (PDF math convention with y-up).
  // CSS rotate() is CW because screen y is down, so the preview transform
  // must negate the angle to visually match the exported PDF.
  it.each([
    [30, '-30'],
    [90, '-90'],
    [145, '-145'],
    [180, '-180'],
    [270, '-270'],
    [359, '-359'],
  ])('negates angle %d for CSS rotate (CCW user → CW screen)', (deg, expected) => {
    const style = stampRotationStyle(deg);
    expect(style.transform).toBe(`rotate(${expected}deg)`);
    expect(style.transformOrigin).toBe('center');
  });
});
