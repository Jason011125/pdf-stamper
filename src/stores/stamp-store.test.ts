import { describe, it, expect, beforeEach } from 'vitest';
import {
  useStampStore,
  DEFAULT_STAMP_CONFIG,
  type StampConfig,
} from './stamp-store';
import { usePdfStore, type PdfFile } from './pdf-store';

function reset(): void {
  useStampStore.setState({
    defaultConfig: { ...DEFAULT_STAMP_CONFIG },
    overrides: {},
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

describe('stamp-store applyEdit — master-index rule (US-FIX1)', () => {
  beforeEach(reset);

  const PDF_IDS = ['a', 'b', 'c', 'd', 'e'];

  // Rule (a): edits on the master PDF (files[0] in pdf-store) write to
  // defaultConfig — every unoverridden PDF inherits the change.
  it('applyEdit on the master PDF writes default → all unoverridden PDFs inherit', () => {
    loadPdfs(PDF_IDS);
    useStampStore.getState().applyEdit('a', { xPt: 100, yPt: 200 });

    expect(useStampStore.getState().overrides).toEqual({});
    for (const id of PDF_IDS) {
      const eff = useStampStore.getState().getEffectiveConfig(id);
      expect(eff.xPt).toBe(100);
      expect(eff.yPt).toBe(200);
    }
  });

  // The headline regression from the bug report: a second edit on the master
  // PDF (e.g. dragging position then changing rotation) must keep writing to
  // default so all unoverridden PDFs follow. The old bootstrap rule routed the
  // second edit to overrides[master] — that's exactly the bug US-FIX1 fixes.
  it('repeated applyEdits on master keep writing default (the FIX1 regression)', () => {
    loadPdfs(PDF_IDS);
    useStampStore.getState().applyEdit('a', { xPt: 100, yPt: 200 });
    useStampStore.getState().applyEdit('a', { rotationDeg: 45 });
    useStampStore.getState().applyEdit('a', { widthPt: 250 });

    expect(useStampStore.getState().overrides).toEqual({});
    for (const id of PDF_IDS) {
      const eff = useStampStore.getState().getEffectiveConfig(id);
      expect(eff.xPt).toBe(100);
      expect(eff.yPt).toBe(200);
      expect(eff.rotationDeg).toBe(45);
      expect(eff.widthPt).toBe(250);
    }
  });

  // Rule (b): edits on a non-master PDF write to overrides[pdfId] only.
  it('applyEdit on a non-master PDF writes overrides[pdfId] → only that PDF shifts', () => {
    loadPdfs(PDF_IDS);
    useStampStore.getState().applyEdit('a', { xPt: 100, yPt: 200 });
    useStampStore.getState().applyEdit('c', { xPt: 333, yPt: 444 });

    expect(useStampStore.getState().overrides).toEqual({
      c: { xPt: 333, yPt: 444 },
    });

    expect(useStampStore.getState().getEffectiveConfig('a').xPt).toBe(100);
    expect(useStampStore.getState().getEffectiveConfig('b').xPt).toBe(100);
    expect(useStampStore.getState().getEffectiveConfig('c').xPt).toBe(333);
    expect(useStampStore.getState().getEffectiveConfig('c').yPt).toBe(444);
    expect(useStampStore.getState().getEffectiveConfig('d').xPt).toBe(100);
    expect(useStampStore.getState().getEffectiveConfig('e').xPt).toBe(100);
  });

  // After the master is removed (e.g. via X-button in the file list, see
  // US-FIX2) files[0] becomes the next PDF; subsequent edits on what is now
  // files[0] write to default. No extra bookkeeping.
  it('master is purely files[0] — removing the master promotes the next PDF', () => {
    loadPdfs(PDF_IDS);
    useStampStore.getState().applyEdit('a', { xPt: 100 });
    expect(useStampStore.getState().defaultConfig.xPt).toBe(100);

    // Drop 'a'; 'b' becomes the new master.
    loadPdfs(['b', 'c', 'd', 'e']);

    useStampStore.getState().applyEdit('b', { xPt: 250 });
    expect(useStampStore.getState().defaultConfig.xPt).toBe(250);
    expect(useStampStore.getState().overrides).toEqual({});

    // Editing 'c' (non-master) again routes to override.
    useStampStore.getState().applyEdit('c', { xPt: 700 });
    expect(useStampStore.getState().overrides).toEqual({ c: { xPt: 700 } });
  });

  it('applyEdit normalizes rotationDeg on both master and non-master paths', () => {
    loadPdfs(PDF_IDS);
    useStampStore.getState().applyEdit('a', { rotationDeg: -30 });
    expect(useStampStore.getState().defaultConfig.rotationDeg).toBe(330);

    useStampStore.getState().applyEdit('b', { rotationDeg: 450 });
    expect(useStampStore.getState().getEffectiveConfig('b').rotationDeg).toBe(90);
    expect(useStampStore.getState().overrides.b).toEqual({ rotationDeg: 90 });
  });

  it('applyEdit shallow-merges into an existing override entry', () => {
    loadPdfs(PDF_IDS);
    useStampStore.getState().applyEdit('b', { xPt: 100, yPt: 50 });
    useStampStore.getState().applyEdit('b', { yPt: 75 });

    expect(useStampStore.getState().overrides.b).toEqual({
      xPt: 100,
      yPt: 75,
    });
  });

  it('applyEdit with empty pdfId is a no-op (no PDFs loaded yet)', () => {
    // No loadPdfs call; files[] is empty.
    useStampStore.getState().applyEdit('', { xPt: 100 });
    expect(useStampStore.getState().defaultConfig.xPt).toBe(
      DEFAULT_STAMP_CONFIG.xPt,
    );
    expect(useStampStore.getState().overrides).toEqual({});
  });
});
