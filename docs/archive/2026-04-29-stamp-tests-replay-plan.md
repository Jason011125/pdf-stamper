# Plan: Stable Stamp Placement Verification

## Goal

Replace the current **isolated** matrix tests in `src-tauri/src/pdf.rs` with **end-to-end content-stream replay** tests that verify the stamped PDF actually places the stamp at the user-picked display rectangle — regardless of whether the rendering engine ever changes.

The existing tests assert the value returned by `image_cm_for_rotation` and `coord_cm_for_rotation` directly. Commit `c6a9a80` shipped a wrong derivation that passed those tests because the unit-test expectations encoded the same wrong derivation. The new tests must assert on the **stamped PDF as a whole**, so the assertion is true regardless of internal refactors.

This is the same approach Adobe Preflight, pikepdf's `getImageCTM()`, and PyMuPDF use: parse the content stream, replay graphics state, ask "where did this object actually land".

## Non-goals

- Do **not** introduce visual regression / golden-image testing.
- Do **not** fix the transparent-PNG → white-background bug (separate issue).
- Do **not** refactor `stamp_image` / `stamp_text` / `image_cm_for_rotation` themselves. The current math is correct — we are upgrading the tests around it.
- Do **not** delete `image_cm_for_rotation` or `coord_cm_for_rotation`. The new tests exercise them indirectly through `stamp_image` / `stamp_text`.

## Why content-stream replay

| Failure mode | Old isolated tests | New replay tests |
|---|---|---|
| Wrong matrix derivation (c6a9a80) | Caught only if test expectation was also right | Always caught — assertion is on observable PDF state |
| Form `/Matrix` interferes with cm | Not caught | Caught (CTM accumulates Form's `/Matrix`) |
| `q/Q` stack imbalance leaking outer state | Not caught | Caught (stack tracker rejects unbalanced) |
| Form `/BBox` truncates the image | Not caught | Caught (assert BBox ⊇ unit square's image) |
| Stamp XObject not registered in Resources | Already caught by `is_ok()` | Still caught |
| Text stamp landing at wrong position on rotated pages | Not caught (CLAUDE.md flags this as active bug) | Caught |

## Files touched

- `src-tauri/src/pdf.rs` — extend `mod tests` with a new helper module. **Production code (lines 1–670) is not modified.**

## Implementation

### Step 1 — `parse_stamp_placement` content-stream walker

Add inside `mod tests`. Returns the resolved CTM at the point a `Do StampN` operator is executed on the page.

```rust
struct StampPlacement {
    /// Name of the stamp Form XObject (e.g. "Stamp7").
    name: String,
    /// CTM at the moment the Form is invoked, in raw page space.
    /// Stored row-major as [a, b, c, d, e, f] like cm.
    page_ctm: [f32; 6],
    /// The Form's own /Matrix (defaults to identity if absent).
    form_matrix: [f32; 6],
    /// The Form's /BBox = [llx, lly, urx, ury].
    form_bbox: [f32; 4],
    /// The cm applied *inside* the Form, immediately before `Do Img0`
    /// (image stamps) or before `BT` (text stamps).
    inner_cm: [f32; 6],
}

fn parse_stamp_placement(pdf_bytes: &[u8]) -> StampPlacement;
```

Algorithm:
1. Load with `lopdf::Document::load_mem`.
2. Get the first page's `/Contents`. It can be either a single stream reference or an array of streams. Concatenate in order, decompress each.
3. Tokenize PDF content-stream operators. **Do not write a full PDF parser** — only the operators we need:
   - `q` (push gstate), `Q` (pop gstate)
   - `cm` (pre-multiply CTM: `CTM := CTM × cm_args`)
   - `Do` — the operand is the next-most-recent `/Name` token
4. Maintain a `Vec<[f32; 6]>` graphics-state stack starting with identity. The current CTM is the top.
5. When you see `Do /StampNNN`, record `page_ctm = current CTM`, then look up the XObject:
   - Page `/Resources/XObject/StampNNN` → resolve to a Stream
   - Read its dict's `/Matrix` (default identity), `/BBox`
6. Recurse one level into the Form's stream: walk it the same way, find either:
   - `Do /Img0` (image stamp) — record the `inner_cm` that was last applied before this `Do`
   - `BT` (text stamp) — record `inner_cm` and the `Td`/`Tm`/`Tj` arguments separately (Step 4)

Tokenizer details and pitfalls:
- Operator/operand separation is whitespace, but operands include strings (`(...)` with balanced parens, watch escapes), hex strings (`<...>`), and names (`/foo`). For our test PDFs we control the content, so a minimal tokenizer that handles `whitespace | name | number | (string) | <hex>` is sufficient. Reject if it sees something it doesn't expect — better than silently mis-parsing.
- `cm` is **pre-multiplication**: `CTM_new = cm_args × CTM_old` if you treat points as column vectors, or `CTM_new = CTM_old × cm_args` for row vectors. PDF 1.7 §8.4.4 defines it as `CTM := M × CTM`. Pick one convention and stick with it; verify with a test PDF that has a known nested `cm`.
- Indirect `/Contents` references must be dereferenced. Some streams have `/Filter /FlateDecode` — decompress with `Stream::decompress()` (already used in `extract_cm_from_stamped`).
- Form XObject's own `/Matrix` is applied to the stream's local coordinates **before** the outer page CTM concatenation. `effective_image_cm = page_ctm × form_matrix × inner_cm`.

**Acceptance for Step 1**: write 2 sanity tests:
1. Stamp a 100×50 image at (10, 20) on a 612×792 unrotated page → `effective_image_cm` applied to (0,0) and (1,1) gives raw-space (10,20) and (110,70).
2. Same on a `/Rotate=90` page → after applying `T_rotate`, corners map to display-space (10,20) and (110,70).

### Step 2 — `assert_stamp_renders_at_display` helper

```rust
fn assert_stamp_renders_at_display(
    stamped_pdf: &[u8],
    rotation: u32,
    raw_w: f32, raw_h: f32,
    expected_dx: f32, expected_dy: f32,
    expected_w: f32, expected_h: f32,
) {
    let p = parse_stamp_placement(stamped_pdf);
    let cm = compose([p.page_ctm, p.form_matrix, p.inner_cm]);

    for (unit, expected_disp) in [
        ((0.0, 0.0), (expected_dx,           expected_dy)),
        ((1.0, 0.0), (expected_dx + expected_w, expected_dy)),
        ((0.0, 1.0), (expected_dx,           expected_dy + expected_h)),
        ((1.0, 1.0), (expected_dx + expected_w, expected_dy + expected_h)),
    ] {
        let raw = apply_cm(cm, unit);
        let disp = apply_t_rotate(rotation, raw, raw_w, raw_h);
        assert_close(disp, expected_disp, 1e-3);
    }

    // Sanity: Form BBox must contain the unit square the image draws into.
    let [llx, lly, urx, ury] = p.form_bbox;
    assert!(llx <= 0.0 && lly <= 0.0 && urx >= 1.0 && ury >= 1.0,
        "Form BBox {:?} does not cover unit square", p.form_bbox);
}
```

`apply_cm` and `apply_t_rotate` already exist in `mod tests` (lines 843, 848). Reuse them.

`compose` should be a 3-way matrix multiply consistent with the convention chosen in Step 1.

### Step 3 — Replace existing image-stamp tests

Delete:
- `extract_cm_from_stamped` (line 738) — superseded
- `assert_stamp_corners_at_display` (line 860) — superseded for end-to-end use
- `image_stamp_corners_no_rotation` / `_rotation_90` / `_rotation_180` / `_rotation_270` / `_landscape_page_rotation_90` (lines 885–917) — these test `image_cm_for_rotation` in isolation
- `cm_vec_to_array` (line 978)

Replace with end-to-end versions that build a real stamped PDF and call `assert_stamp_renders_at_display`:

```rust
#[test]
fn stamp_image_lands_at_display_no_rotation() {
    let pdf = make_test_pdf(612.0, 792.0, None);
    let png = make_red_png();
    let out = stamp_image(&pdf, &png, 100.0, 200.0, 150.0, 75.0).unwrap();
    assert_stamp_renders_at_display(&out, 0, 612.0, 792.0, 100.0, 200.0, 150.0, 75.0);
}
// + 90, 180, 270, and a landscape-page-rotated-90 case
```

The existing `stamp_image_lands_at_display_*` tests at lines 983–1010 already exist with this shape but call `extract_cm_from_stamped` (which only reads the inner Form's cm and ignores Form `/Matrix` and any outer `q/cm`). Update them to call `assert_stamp_renders_at_display`.

### Step 4 — Text-stamp placement assertion

Add a parser branch and a helper:

```rust
struct TextPlacement {
    page_ctm: [f32; 6],
    form_matrix: [f32; 6],
    inner_cm: [f32; 6],   // cm applied before BT
    text_origin: (f32, f32), // from Td or Tm
    font_size: f32,
}

fn parse_text_placement(pdf_bytes: &[u8]) -> TextPlacement;

fn assert_text_renders_at_display(
    stamped_pdf: &[u8],
    rotation: u32,
    raw_w: f32, raw_h: f32,
    expected_dx: f32, expected_dy: f32,
    expected_font_size: f32,
);
```

Inside the Form stream the operator sequence is `q [cm] [rg] BT Tf Td Tj ET Q` (see `stamp_text` at line 372). Track:
- `inner_cm` = the `cm` between `q` and `BT` (or identity if absent for `/Rotate=0`)
- After `BT`, the *text matrix* `T_m` and *text line matrix* `T_lm` reset to identity. `Td x y` does `T_m = T_lm = [1, 0, 0, 1, x, y] × T_lm`. We don't have `Tm` in the current generator, so just read the `Td` args.
- `Tf font size` gives font size

The text's origin in **raw space** is `apply_cm(page_ctm × form_matrix × inner_cm × text_matrix, (0, 0))`. Run that through `T_rotate` and assert it equals `(expected_dx, expected_dy)` within tolerance.

Update existing text tests (lines 1178–1196) to call this. Add new tests for `/Rotate=180` and `/Rotate=270`. **One of these will fail until the bug noted in CLAUDE.md (`coord_cm_for_rotation`) is fixed** — that is the point. If they all pass, the parser is wrong; double-check by hand-verifying the numbers for the 270° case.

### Step 5 — Smoke check on a real-world PDF

Keep the existing `diagnose_real_pdf` test (line 1024) but switch its assertions from `extract_cm_from_stamped` to `assert_stamp_renders_at_display`. If it's currently `#[ignore]`d for being slow, leave it ignored.

## Acceptance criteria

1. `cd src-tauri && cargo test` passes.
2. `extract_cm_from_stamped`, `assert_stamp_corners_at_display`, `cm_vec_to_array` are removed (no other references).
3. The four `image_cm_for_rotation` isolated property tests are gone; only the end-to-end `stamp_image_lands_at_display_*` tests remain.
4. Text-stamp tests now assert position, not just `is_ok()`.
5. No production code outside `mod tests` is changed.
6. To prove the new tests catch the original bug: temporarily revert `image_cm_for_rotation` to the broken c6a9a80 version (commit `git show c6a9a80:src-tauri/src/pdf.rs`); the new tests must fail. Restore before committing.

## PDF spec references

- ISO 32000-1 (PDF 1.7) §7.8 — Content Streams and Resources
- §8.4 — Graphics State (q, Q, cm)
- §8.9.5 — Image XObjects (unit-square mapping — already covered in CLAUDE.md)
- §8.10 — Form XObjects (`/Matrix` and `/BBox` semantics)
- §9.4 — Text Object Operators (BT, ET, Tm, Td, Tj, Tf)

## Out of scope (for this PR)

- Transparent PNG → SMask fix
- Output filename collisions
- Batch progress events
- pdfium render-based smoke tests for alpha / SMask correctness — these need pixel sampling and are a separate concern from "stamp lands at the right position".
