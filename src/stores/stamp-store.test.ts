import { describe, it, expect, beforeEach } from 'vitest';
import {
  useStampStore,
  DEFAULT_STAMP_CONFIG,
  type StampConfig,
} from './stamp-store';

function reset(): void {
  useStampStore.setState({
    defaultConfig: { ...DEFAULT_STAMP_CONFIG },
    overrides: {},
    hasBootstrappedDefault: false,
  });
}

describe('stamp-store rotationDeg (via setRotationDeg shim → setDefault)', () => {
  beforeEach(reset);

  it('defaults to 0', () => {
    expect(useStampStore.getState().defaultConfig.rotationDeg).toBe(0);
  });

  it('stores values within [0, 360) unchanged', () => {
    const { setRotationDeg } = useStampStore.getState();
    setRotationDeg(45);
    expect(useStampStore.getState().defaultConfig.rotationDeg).toBe(45);
    setRotationDeg(0);
    expect(useStampStore.getState().defaultConfig.rotationDeg).toBe(0);
    setRotationDeg(359.5);
    expect(useStampStore.getState().defaultConfig.rotationDeg).toBe(359.5);
  });

  it('normalizes 360 to 0', () => {
    useStampStore.getState().setRotationDeg(360);
    expect(useStampStore.getState().defaultConfig.rotationDeg).toBe(0);
  });

  it('wraps values >= 360 into [0, 360)', () => {
    useStampStore.getState().setRotationDeg(450);
    expect(useStampStore.getState().defaultConfig.rotationDeg).toBe(90);
    useStampStore.getState().setRotationDeg(720);
    expect(useStampStore.getState().defaultConfig.rotationDeg).toBe(0);
  });

  it('wraps negative values into [0, 360)', () => {
    useStampStore.getState().setRotationDeg(-30);
    expect(useStampStore.getState().defaultConfig.rotationDeg).toBe(330);
    useStampStore.getState().setRotationDeg(-90);
    expect(useStampStore.getState().defaultConfig.rotationDeg).toBe(270);
    useStampStore.getState().setRotationDeg(-360);
    expect(useStampStore.getState().defaultConfig.rotationDeg).toBe(0);
  });

  it('falls back to 0 on NaN/Infinity', () => {
    useStampStore.getState().setRotationDeg(Number.NaN);
    expect(useStampStore.getState().defaultConfig.rotationDeg).toBe(0);
    useStampStore.getState().setRotationDeg(Number.POSITIVE_INFINITY);
    expect(useStampStore.getState().defaultConfig.rotationDeg).toBe(0);
  });

  it('normalizes rotationDeg passed via setOverride too', () => {
    useStampStore.getState().setOverride('p1', { rotationDeg: -45 });
    expect(useStampStore.getState().getEffectiveConfig('p1').rotationDeg).toBe(315);
    useStampStore.getState().setOverride('p1', { rotationDeg: Number.NaN });
    expect(useStampStore.getState().getEffectiveConfig('p1').rotationDeg).toBe(0);
  });
});

describe('stamp-store model A — defaultConfig + per-PDF overrides', () => {
  beforeEach(reset);

  // Invariant 1: default exists from start
  it('starts with defaultConfig populated and overrides empty', () => {
    const state = useStampStore.getState();
    expect(state.defaultConfig).toEqual(DEFAULT_STAMP_CONFIG);
    expect(state.overrides).toEqual({});
    expect(state.getEffectiveConfig('any-pdf')).toEqual(DEFAULT_STAMP_CONFIG);
  });

  // Invariant 2: override never modifies default
  it('setOverride leaves defaultConfig untouched', () => {
    const before: StampConfig = { ...useStampStore.getState().defaultConfig };
    useStampStore.getState().setOverride('p1', { xPt: 200, yPt: 150 });
    expect(useStampStore.getState().defaultConfig).toEqual(before);
    expect(useStampStore.getState().getEffectiveConfig('p1')).toMatchObject({
      ...before,
      xPt: 200,
      yPt: 150,
    });
  });

  // Invariant 3: changing default affects all PDFs without an override
  it('setDefault flows through to every PDF that has no override', () => {
    useStampStore.getState().setOverride('p2', { xPt: 999 });
    useStampStore.getState().setDefault({ color: '#00ff00', widthPt: 250 });

    const eff0 = useStampStore.getState().getEffectiveConfig('p0');
    const eff1 = useStampStore.getState().getEffectiveConfig('p1');
    const eff2 = useStampStore.getState().getEffectiveConfig('p2');

    expect(eff0.color).toBe('#00ff00');
    expect(eff0.widthPt).toBe(250);
    expect(eff1.color).toBe('#00ff00');
    expect(eff1.widthPt).toBe(250);
    expect(eff2.color).toBe('#00ff00');
    expect(eff2.widthPt).toBe(250);
  });

  // Invariant 4: PDF with override stays put when default changes
  it('overridden fields are not touched by setDefault', () => {
    useStampStore.getState().setOverride('p1', { xPt: 500, yPt: 400 });
    useStampStore.getState().setDefault({ xPt: 10, yPt: 20, color: '#0000ff' });

    const eff = useStampStore.getState().getEffectiveConfig('p1');
    expect(eff.xPt).toBe(500); // override wins
    expect(eff.yPt).toBe(400); // override wins
    expect(eff.color).toBe('#0000ff'); // not overridden → falls through
  });

  // Invariant 5: clearing override returns that PDF to default
  it('clearOverride drops the entry and the PDF reverts to default', () => {
    useStampStore.getState().setOverride('p1', { xPt: 500 });
    expect(useStampStore.getState().getEffectiveConfig('p1').xPt).toBe(500);

    useStampStore.getState().clearOverride('p1');
    expect(useStampStore.getState().overrides['p1']).toBeUndefined();
    expect(useStampStore.getState().getEffectiveConfig('p1')).toEqual(
      DEFAULT_STAMP_CONFIG,
    );
  });

  it('setOverride shallow-merges into existing override entry', () => {
    useStampStore.getState().setOverride('p1', { xPt: 100, yPt: 50 });
    useStampStore.getState().setOverride('p1', { yPt: 75 });

    expect(useStampStore.getState().overrides['p1']).toEqual({
      xPt: 100,
      yPt: 75,
    });
    const eff = useStampStore.getState().getEffectiveConfig('p1');
    expect(eff.xPt).toBe(100);
    expect(eff.yPt).toBe(75);
  });

  it('setDefault shallow-merges (does not replace untouched fields)', () => {
    const before = useStampStore.getState().defaultConfig.color;
    useStampStore.getState().setDefault({ xPt: 17 });
    expect(useStampStore.getState().defaultConfig.xPt).toBe(17);
    expect(useStampStore.getState().defaultConfig.color).toBe(before);
  });

  it('clearOverride is a no-op when the pdfId has no override', () => {
    useStampStore.getState().clearOverride('never-overridden');
    expect(useStampStore.getState().overrides).toEqual({});
  });

  it('getEffectiveConfig returns a fresh object (not the underlying default)', () => {
    const a = useStampStore.getState().getEffectiveConfig('p0');
    const b = useStampStore.getState().getEffectiveConfig('p0');
    expect(a).toEqual(b);
    expect(a).not.toBe(useStampStore.getState().defaultConfig);
  });
});

describe('stamp-store applyEdit — override-on-edit (US-P2)', () => {
  beforeEach(reset);

  const PDF_IDS = ['p0', 'p1', 'p2', 'p3', 'p4'];

  it('starts with hasBootstrappedDefault=false', () => {
    expect(useStampStore.getState().hasBootstrappedDefault).toBe(false);
  });

  // (a) From the AC: first edit on PDF 0 → all 5 effective configs match
  // because the bootstrap call writes to defaultConfig.
  it('first applyEdit writes default → every PDF inherits the change', () => {
    useStampStore.getState().applyEdit('p0', { xPt: 100, yPt: 200 });

    expect(useStampStore.getState().hasBootstrappedDefault).toBe(true);
    expect(useStampStore.getState().overrides).toEqual({});

    for (const id of PDF_IDS) {
      const eff = useStampStore.getState().getEffectiveConfig(id);
      expect(eff.xPt).toBe(100);
      expect(eff.yPt).toBe(200);
    }
  });

  // (b) From the AC: after the bootstrap, an edit on PDF 2 → only PDF 2 shifts.
  it('subsequent applyEdit writes overrides[pdfId] → only that PDF shifts', () => {
    useStampStore.getState().applyEdit('p0', { xPt: 100, yPt: 200 });
    useStampStore.getState().applyEdit('p2', { xPt: 333, yPt: 444 });

    expect(useStampStore.getState().overrides).toEqual({
      p2: { xPt: 333, yPt: 444 },
    });

    expect(useStampStore.getState().getEffectiveConfig('p0').xPt).toBe(100);
    expect(useStampStore.getState().getEffectiveConfig('p1').xPt).toBe(100);
    expect(useStampStore.getState().getEffectiveConfig('p2').xPt).toBe(333);
    expect(useStampStore.getState().getEffectiveConfig('p2').yPt).toBe(444);
    expect(useStampStore.getState().getEffectiveConfig('p3').xPt).toBe(100);
    expect(useStampStore.getState().getEffectiveConfig('p4').xPt).toBe(100);
  });

  // (c) From the AC: setDefault again → unoverridden PDFs follow, overridden PDF stays.
  it('setDefault after an override flows to unoverridden PDFs only', () => {
    useStampStore.getState().applyEdit('p0', { xPt: 100, yPt: 200 });
    useStampStore.getState().applyEdit('p2', { xPt: 333, yPt: 444 });

    useStampStore.getState().setDefault({ xPt: 999, yPt: 888 });

    expect(useStampStore.getState().getEffectiveConfig('p0').xPt).toBe(999);
    expect(useStampStore.getState().getEffectiveConfig('p1').xPt).toBe(999);
    expect(useStampStore.getState().getEffectiveConfig('p2').xPt).toBe(333); // override wins
    expect(useStampStore.getState().getEffectiveConfig('p2').yPt).toBe(444); // override wins
    expect(useStampStore.getState().getEffectiveConfig('p3').xPt).toBe(999);
    expect(useStampStore.getState().getEffectiveConfig('p4').xPt).toBe(999);
  });

  // Subsequent edits to the *same* PDF that received the bootstrap go to that
  // PDF's override (the bootstrap rule is global, not per-PDF). This is the
  // gotcha the AC notes ("first edit goes to default; subsequent edits on the
  // same PDF go to its override").
  it('second applyEdit on the bootstrap PDF writes its override, not default', () => {
    useStampStore.getState().applyEdit('p0', { xPt: 100 });
    useStampStore.getState().applyEdit('p0', { xPt: 200 });

    expect(useStampStore.getState().defaultConfig.xPt).toBe(100); // bootstrap value
    expect(useStampStore.getState().overrides.p0).toEqual({ xPt: 200 });
    expect(useStampStore.getState().getEffectiveConfig('p0').xPt).toBe(200);

    // Other PDFs still see the bootstrap value
    expect(useStampStore.getState().getEffectiveConfig('p1').xPt).toBe(100);
  });

  it('applyEdit normalizes rotationDeg in both bootstrap and override paths', () => {
    useStampStore.getState().applyEdit('p0', { rotationDeg: -30 });
    expect(useStampStore.getState().defaultConfig.rotationDeg).toBe(330);

    useStampStore.getState().applyEdit('p1', { rotationDeg: 450 });
    expect(useStampStore.getState().getEffectiveConfig('p1').rotationDeg).toBe(90);
  });

  it('applyEdit shallow-merges into an existing override entry', () => {
    useStampStore.getState().applyEdit('p0', { xPt: 0 }); // bootstrap (drains the flag)
    useStampStore.getState().applyEdit('p1', { xPt: 100, yPt: 50 });
    useStampStore.getState().applyEdit('p1', { yPt: 75 });

    expect(useStampStore.getState().overrides.p1).toEqual({
      xPt: 100,
      yPt: 75,
    });
  });
});
