# Ralph Loop — Export Improvements

Two related changes to the PDF export pipeline. Tasks are independent and ordered.

## Loop protocol
- Re-read this file at the start of every iteration.
- Pick the **first unchecked** task.
- Execute end-to-end: code + Rust tests + manual smoke.
- Tick `[x]` and add a one-line status note under the task.
- Commit (one task = one commit, conventional commits).
- Stop when every checkbox is `[x]` **and** the manual smoke checklist passes.
- If blocked, append the blocker under **Blockers** and halt — do not guess.

---

## Task 1 — Drop `-stamped` suffix from output filenames

**What**: Output PDFs currently save as `{name}-stamped.pdf`. Change to `{name}.pdf`.

**Why**: User request. Output dir is user-selected and almost always distinct from input dir, so collisions are rare. But the change makes them more likely, so we also need a collision guard.

**Where**:
- `src-tauri/src/commands.rs` — `stamp_pdfs` handler, look for `format!("{}-stamped.pdf", …)` (could be in `pdf.rs` instead — grep both).

**How**:
1. Replace the suffix template so output is `{stem}.pdf`.
2. **Refuse to overwrite the input**: if computed output path == input path, return an `Err` ("Output directory must differ from the source — would overwrite `{path}`."). Do **not** silently clobber.
3. **Dedup same-basename inputs**: if two input files share a basename (e.g. `a/foo.pdf` and `b/foo.pdf`), the second one must save as `foo (2).pdf`, third as `foo (3).pdf`, etc. Tracks the existing known-issue note in `CLAUDE.md`.

**Tests** (`src-tauri/src/pdf.rs` or a new helper module — match where filename logic ends up):
- `output_filename_drops_stamped_suffix` — `foo.pdf` → `foo.pdf`.
- `output_filename_dedups_same_basename` — `[a/foo.pdf, b/foo.pdf]` → `[foo.pdf, foo (2).pdf]`.
- `stamp_pdfs_refuses_to_overwrite_input` — output dir == input dir → returns error, source file untouched on disk.

**Manual smoke**:
- Open two PDFs from different folders with the same basename. Export to a third folder. Confirm `foo.pdf` and `foo (2).pdf`, no `-stamped`.
- Try to export to the same folder as one of the inputs. Confirm clear error toast/dialog (frontend should surface the Rust error).

**Checkboxes**:
- [x] Suffix removed
- [x] Overwrite-input guard returns `Err`
- [x] Same-basename dedup implemented
- [x] Three Rust tests added and passing (`cd src-tauri && cargo test`)
- [ ] Manual smoke verified _(needs the user — can't drive the file picker from here)_

**Status**: code + 38 Rust / 75 vitest tests green. `STAMP_SUFFIX` removed; `compute_output_paths` adds `(2)`/`(3)` suffix on basename collisions; `would_overwrite_input` precheck refuses input==output before any write happens. IPC drops the `suffix` arg end-to-end (Rust → bridge → tests → component).

---

## Task 2 — "Flatten to scanned PDF" export mode

**What**: After the existing `stamp_image` / `stamp_text` produces the stamped PDF bytes, optionally rasterize the page and emit a new single-page PDF whose only content is that raster image. The stamp becomes pixels — uneditable in WPS/Acrobat PDF edit mode.

**Why**: User opens current output in WPS → "PDF Edit" → can grab and move the stamp because it's a Form XObject. They want a "scanned-type PDF" where stamp + page are merged into one image layer.

### Design

UI:
- New checkbox in `src/components/stamp-controls.tsx`: **"Flatten output (stamp becomes uneditable)"**. **Default ON**.
- Persist in `src/stores/stamp-store.ts` as `flatten: boolean`.
- Pipe through `src/services/pdf-bridge.ts` → `stamp_pdfs` IPC → Rust.

Rust pipeline (when `flatten=true`):
1. Run the existing stamp path → in-memory `Vec<u8>` of stamped PDF.
2. Open with `pdfium-render`.
3. Render page 0 at **300 DPI** with rotation applied (pdfium default — confirm with a quick probe; the rendered image must already be in **display** orientation).
4. Composite over white background, convert to RGB8 (drop alpha).
5. Encode as JPEG, quality **92**, via the `image` crate's `JpegEncoder` (already a dep — no new crates).
6. Build a fresh single-page PDF with `lopdf`:
   - `/MediaBox` = display dimensions in points (swap w/h vs. raw if original `/Rotate` ∈ {90°, 270°}).
   - **No** `/Rotate` on the new page (the raster is already in display orientation).
   - Resources: one Image XObject `/Im0` with `/Filter /DCTDecode`, `/Width`, `/Height` matching the JPEG, `/ColorSpace /DeviceRGB`, `/BitsPerComponent 8`.
   - Content stream: `q {page_w} 0 0 {page_h} 0 0 cm /Im0 Do Q`.
7. Return the new PDF bytes; existing save logic writes them out.

**Where**:
- `src-tauri/src/pdf.rs` — new `flatten_pdf_to_scanned(stamped_bytes: &[u8], dpi: f32, jpeg_quality: u8) -> Result<Vec<u8>, …>`.
- `src-tauri/src/commands.rs` — extend `stamp_pdfs` signature with `flatten: bool`. Call `flatten_pdf_to_scanned` after the stamp step when `flatten` is true.
- `src/services/pdf-bridge.ts` — add `flatten` arg to the `stampAllPdfs` wrapper.
- `src/stores/stamp-store.ts` — add `flatten: boolean` (default `true`) and a setter.
- `src/components/stamp-controls.tsx` — render the toggle, wire to the store.

### Tests (Rust, `src-tauri/src/pdf.rs`)

- `flatten_output_has_no_form_xobject` — walk the output PDF object tree, assert **no** Form XObject anywhere (Subtype `/Form`). Page Resources must contain exactly one XObject of Subtype `/Image`.
- `flatten_output_image_uses_dct_filter` — that single Image XObject's stream `/Filter` is `/DCTDecode`.
- `flatten_preserves_display_dimensions_no_rotate` — A4 portrait input (595×842 pt, no `/Rotate`) → output `/MediaBox` is `[0 0 595 842]` (±1pt) and output has no `/Rotate` entry.
- `flatten_preserves_display_dimensions_with_rotate_90` — A4 portrait input with `/Rotate 90` → output `/MediaBox` is `[0 0 842 595]` (±1pt), no `/Rotate`.
- `flatten_visual_smoke_red_stamp` — synthetic single-page PDF, `stamp_image` a known red square at a known position with `flatten=true`, render output back via pdfium, assert >95% red pixels inside the expected display-space bbox and ~0% red outside it.
- All existing tests still pass with `flatten=false` (i.e. nothing changes when toggle is off).

### Manual smoke

1. Export an input with **flatten OFF**. Open in WPS → "PDF Edit". Confirm stamp is selectable and movable (= current behavior, sanity check).
2. Export the same input with **flatten ON**. Open in WPS → "PDF Edit". Confirm:
   - Stamp **cannot** be selected as a separate object.
   - Whole page behaves as one image.
3. File size sanity: A4-ish input at 300 DPI JPEG q=92 should land ~300 KB – 2 MB depending on content.
4. Visual quality: zoom 200% in viewer, no objectionable JPEG artifacts on text.

### Checkboxes

- [ ] `flatten_pdf_to_scanned` implemented in `pdf.rs`
- [ ] Display-orientation handling verified for `/Rotate` ∈ {0°, 90°, 180°, 270°}
- [ ] `stamp_pdfs` accepts and threads the `flatten` flag
- [ ] `stampAllPdfs` IPC wrapper updated
- [ ] `stamp-store` has `flatten` (default `true`) + setter
- [ ] `stamp-controls.tsx` toggle wired
- [ ] All five Rust tests above written and passing
- [ ] Existing `cargo test` suite green
- [ ] `npx tsc --noEmit` clean
- [ ] WPS smoke test passes (stamp uneditable when flatten=on)

---

## Files to read first (one pass before starting Task 1)

- `src-tauri/src/commands.rs` — `stamp_pdfs` handler
- `src-tauri/src/pdf.rs` — `stamp_image`, `stamp_text`, `render_page_to_png`, `get_page_geometry`
- `src/services/pdf-bridge.ts` — `stampAllPdfs`
- `src/stores/stamp-store.ts` — current shape
- `src/components/stamp-controls.tsx` — export button

## Conventions (don't violate)

- Match existing casing: Rust snake_case, TS camelCase, files kebab-case.
- Errors: `Result<T, E>` with `thiserror`.
- No `console.log`. No drive-by refactors. No comments unless WHY is non-obvious.
- One commit per task. Conventional: `feat:`, `fix:`, `test:`.
- Don't introduce new crates — `lopdf`, `pdfium-render`, `image` are sufficient.

## Definition of Done

- Both task sections fully checked.
- `cd src-tauri && cargo test` green.
- `npx tsc --noEmit` clean.
- `npm run tauri dev` boots without runtime errors.
- Both manual smoke checklists pass.

## Blockers
_(append here and halt the loop if you find one)_
