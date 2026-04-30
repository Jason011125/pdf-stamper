import { describe, it, expect, beforeEach } from 'vitest';
import { useStampStore } from './stamp-store';

function reset(): void {
  useStampStore.setState({ rotationDeg: 0 });
}

describe('stamp-store rotationDeg', () => {
  beforeEach(reset);

  it('defaults to 0', () => {
    expect(useStampStore.getState().rotationDeg).toBe(0);
  });

  it('stores values within [0, 360) unchanged', () => {
    const { setRotationDeg } = useStampStore.getState();
    setRotationDeg(45);
    expect(useStampStore.getState().rotationDeg).toBe(45);
    setRotationDeg(0);
    expect(useStampStore.getState().rotationDeg).toBe(0);
    setRotationDeg(359.5);
    expect(useStampStore.getState().rotationDeg).toBe(359.5);
  });

  it('normalizes 360 to 0', () => {
    useStampStore.getState().setRotationDeg(360);
    expect(useStampStore.getState().rotationDeg).toBe(0);
  });

  it('wraps values >= 360 into [0, 360)', () => {
    useStampStore.getState().setRotationDeg(450);
    expect(useStampStore.getState().rotationDeg).toBe(90);
    useStampStore.getState().setRotationDeg(720);
    expect(useStampStore.getState().rotationDeg).toBe(0);
  });

  it('wraps negative values into [0, 360)', () => {
    useStampStore.getState().setRotationDeg(-30);
    expect(useStampStore.getState().rotationDeg).toBe(330);
    useStampStore.getState().setRotationDeg(-90);
    expect(useStampStore.getState().rotationDeg).toBe(270);
    useStampStore.getState().setRotationDeg(-360);
    expect(useStampStore.getState().rotationDeg).toBe(0);
  });

  it('falls back to 0 on NaN/Infinity', () => {
    useStampStore.getState().setRotationDeg(Number.NaN);
    expect(useStampStore.getState().rotationDeg).toBe(0);
    useStampStore.getState().setRotationDeg(Number.POSITIVE_INFINITY);
    expect(useStampStore.getState().rotationDeg).toBe(0);
  });
});
