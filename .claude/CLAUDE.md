# PDF Stamper

Batch tool for adding stamps (images/text) to single-page PDF files without quality loss.

## Tech Stack

- **Framework**: Tauri v2 (Rust backend + web frontend)
- **Frontend**: React 18 + TypeScript + Vite
- **PDF Rendering**: pdfium via `pdfium-render` crate (dynamically loaded from `src-tauri/libs/pdfium/lib`)
- **PDF Manipulation**: lopdf 0.34 (Rust, MIT license) — used for low-level PDF object manipulation
- **Image Processing**: `image` crate 0.25 — decodes stamp images (PNG, JPEG, WebP)
- **State Management**: Zustand
- **Styling**: Tailwind CSS

## Architecture Overview

```
┌─────────────────────────────────────────────────────┐
│                    Tauri Window                      │
│  ┌──────────┐  ┌──────────────┐  ┌───────────────┐  │
│  │ FileList  │  │  PreviewPane │  │ StampControls │  │
│  │ (left)    │  │  (center)    │  │ (right)       │  │
│  └────┬─────┘  └──────┬───────┘  └───────┬───────┘  │
│       │               │                  │           │
│       └───────────┬────┴──────────────────┘           │
│                   │ Zustand Stores                    │
│          ┌────────┴─────────┐                         │
│          │  pdf-store.ts    │  File list + selection   │
│          │  stamp-store.ts  │  Stamp config + position │
│          └────────┬─────────┘                         │
│                   │ IPC (invoke)                      │
├───────────────────┼─────────────────────────────────-─┤
│ Rust Backend      │                                   │
│          ┌────────┴─────────┐                         │
│          │  commands.rs     │  Tauri command handlers  │
│          │  pdf.rs          │  All PDF logic           │
│          │  lib.rs          │  App bootstrap           │
│          └──────────────────┘                         │
└─────────────────────────────────────────────────────┘
```

## Project Structure (Detailed)

```
src-tauri/src/
  lib.rs              — Tauri app bootstrap, registers IPC commands
  commands.rs         — Tauri IPC handlers (open_pdfs, render_page, read_file_bytes, stamp_pdfs)
  pdf.rs              — Core PDF logic:
                        • get_page_geometry()  — reads MediaBox, /Rotate, inherits from parent nodes
                        • get_page_dimensions() — convenience wrapper returning effective dimensions
                        • render_page_to_png()  — renders first page via pdfium
                        • stamp_image()         — overlays image stamp using Form XObject + cm matrix
                        • stamp_text()          — overlays text stamp using Form XObject + BT/ET
                        • create_image_xobject() — encodes stamp image as PDF XObject (JPEG=DCTDecode, others=Flate)
                        • register_xobject()    — registers XObject in page Resources (handles indirect refs)
                        • append_content_stream() — appends stamp content stream to page Contents
                        • parse_hex_color()     — converts "#rrggbb" to (f32, f32, f32)

src/
  App.tsx               — Root layout: 3-column (file list | preview | stamp controls)
  main.tsx              — React entry point

  components/
    file-list.tsx       — PDF file open dialog, file list with selection and removal
    preview-pane.tsx    — PDF page preview, stamp overlay positioning (click/drag)
    stamp-controls.tsx  — Stamp type toggle, image upload, text config, size inputs, export button

  stores/
    pdf-store.ts        — Zustand store: loaded PDF files, selection index, preview URLs
    stamp-store.ts      — Zustand store: stamp type/config, position (xPt, yPt), size (widthPt, heightPt)

  services/
    pdf-bridge.ts       — Tauri invoke wrappers (openPdfDialog, loadPdfs, renderPage, stampAllPdfs)
    coordinate-utils.ts — Coordinate conversion helpers (screenToPdf, pdfToScreen, pdfSizeToScreen)
```

## Data Flow

### Opening PDFs
1. `FileList` → `openPdfDialog()` → native file picker
2. Selected paths → `open_pdfs` IPC → Rust reads each PDF, extracts page dimensions via `get_page_geometry()`
3. Returns `PdfInfo[]` (path, filename, width_pt, height_pt) → stored in `pdf-store`
4. Background: each file → `render_page` IPC → pdfium renders PNG → stored as blob URL in `pdf-store`

### Placing a Stamp
1. User clicks/drags on `PreviewPane` image
2. Screen coordinates → `toPdfPos()` → PDF coordinates (bottom-left origin, points)
3. Position stored as `(xPt, yPt)` in `stamp-store`
4. Stamp overlay rendered at `toScreenPos(xPt, yPt)` for visual feedback

### Exporting
1. `StampControls` → `stampAllPdfs()` → `stamp_pdfs` IPC
2. For each PDF: reads file, calls `stamp_image()` or `stamp_text()` with (x, y, width, height) in PDF points
3. Stamp is added as Form XObject appended to page Contents (original content untouched)
4. Saved to user-selected output directory as `{name}-stamped.pdf`

## Coordinate System

**Critical concept** — two coordinate systems:
- **Screen space**: origin top-left, Y increases downward (pixels)
- **PDF space**: origin bottom-left, Y increases upward (points, 1pt = 1/72 inch)

The store keeps stamp position in **PDF points** (`xPt`, `yPt` = bottom-left corner of stamp).
`preview-pane.tsx` has `toScreenPos()` and `toPdfPos()` to convert between systems.

### Three coordinate spaces in the Rust side

When a PDF has `/Rotate ≠ 0`, three spaces matter:

1. **Image space** — internal to a PDF Image XObject. Origin top-left, Y down. Sample (0, 0) is the source image's top-left pixel.
2. **Raw page space** — what `MediaBox` measures. Origin bottom-left, Y up. This is what gets written into the PDF content stream.
3. **Display space** — what the viewer (and our preview) actually shows. Same handedness as raw, but rotated by `/Rotate` degrees CW.

The user picks stamp positions in **display space**. We store them as `(xPt, yPt, widthPt, heightPt)` in display units. The Rust side converts to **raw space** when emitting the stamp's `cm` matrix.

## PDF Image Stamp `cm` Matrix Recipe

> ⚠️ This is the most error-prone code in the project. The previous implementation had every rotation case wrong (commit c6a9a80 confidently introduced a 180° flip while claiming to fix one). **Read this section before modifying `image_cm_for_rotation` in `pdf.rs`.**

### Image XObject sample → unit-square mapping (PDF 1.7 §8.9.5)

A PDF Image XObject is drawn into the **unit square** in user space (corners at `(0, 0)` and `(1, 1)`). The image's *first* sample (top-left of the source image) lands at unit **(0, 1)**; the *last* sample lands at unit **(1, 0)**. PDF already accounts for the top-down storage vs. bottom-up user space — **no manual Y flip is needed**.

→ The minimal "draw image upright at PDF position `(x, y)` with size `(w, h)`" matrix is just `[w, 0, 0, h, x, y]`. Anything with a negative `d` is wrong (or is doing something exotic).

### `/Rotate` semantics (PDF 1.7 §14.8.2)

`/Rotate` is applied *after* content rendering, by the viewer. So we draw in **raw space**, and the viewer then rotates the whole page CW by `R°` to produce the **display**.

The viewer's transform `T_rotate` (raw → display) for a raw page of size `(raw_w, raw_h)`:

| `R` | `T_rotate(u, v)` |
|-----|------------------|
| 0°   | `(u, v)` |
| 90°  | `(v, raw_w − u)` |
| 180° | `(raw_w − u, raw_h − v)` |
| 270° | `(raw_h − v, u)` |

To put an upright stamp at *display* `(dx, dy)` with size `(w, h)`, we compose:

```
M_total = M_display→raw  ·  M_image→display
        = T_rotate⁻¹     ·  [w, 0, 0, h, dx, dy]
```

### The four cm matrices (`image_cm_for_rotation` in `pdf.rs`)

| `/Rotate` | cm = `[a, b, c, d, e, f]` |
|-----------|-----------------------------|
| 0°   | `[w, 0, 0, h, dx, dy]` |
| 90°  | `[0, w, −h, 0, raw_w − dy, dx]` |
| 180° | `[−w, 0, 0, −h, raw_w − dx, raw_h − dy]` |
| 270° | `[0, −w, h, 0, dy, raw_h − dx]` |

### The page's existing CTM is NOT identity — wrap it

A correct cm in our Form XObject is necessary but not sufficient. Many real-world PDFs (web-app exports, Notion, Google Docs, …) start their content stream with a top-level `cm` that establishes a top-down "screen" coordinate system, e.g. `0.24 0 0 -0.24 0 841 cm`, and **never** balance it inside a `q ... Q`. End-of-stream CTM is therefore non-identity. If we just append our stamp stream, our `cm` post-composes with the leftover CTM and the stamp ends up scaled / Y-flipped / translated.

`append_content_stream` solves this generically by wrapping the existing Contents in `q ... Q`:

```
[q_stream, ...existing Contents..., Q_stream, our_stamp_stream]
```

`q` saves the initial graphics state, the original content runs and may pollute CTM/colors/clipping, `Q` rolls everything back, our stamp then runs from a clean identity state. Independent of what the original PDF does.

Regression covered by `stamp_image_survives_polluted_ctm_in_existing_contents` (synthetic polluted-CTM PDF + pdfium pixel verification). For ad-hoc debug on a customer's PDF, use `diagnose_real_pdf` (`#[ignore]`):

```bash
DIAGNOSE_PDF=/path/to/file.pdf cargo test --lib diagnose_real_pdf -- --ignored --nocapture
```

It dumps the existing Contents, runs `stamp_image` with a known red square, renders the result, and reports red-pixel bbox vs. expected.

### Why pure-arithmetic unit tests aren't enough

The tests that shipped with the broken matrices passed because they compared the function's output against a hand-computed expression that **encoded the same wrong derivation**. The test was effectively `assert(buggy_function() == buggy_expression)`.

**Use property-based tests instead.** For each rotation, verify the stamp's *display-space corners after the viewer's rotation* land where the user picked. The helper pattern:

```rust
fn assert_stamp_lands_at_display(rot, dx, dy, w, h, raw_w, raw_h) {
    let cm = image_cm_for_rotation(rot, dx, dy, w, h, raw_w, raw_h);
    for (unit_corner, expected_display_corner) in [
        ((0.0, 0.0), (dx,     dy)),       // image bottom-left → stamp BL
        ((1.0, 0.0), (dx + w, dy)),       // image bottom-right → stamp BR
        ((0.0, 1.0), (dx,     dy + h)),   // image top-left → stamp TL
        ((1.0, 1.0), (dx + w, dy + h)),   // image top-right → stamp TR
    ] {
        let raw  = apply_cm(cm, unit_corner);
        let disp = apply_t_rotate(rot, raw, raw_w, raw_h);
        assert_close(disp, expected_display_corner);
    }
}
```

This tests the *property we care about* (stamp lands at the user-picked display rectangle, upright) rather than the matrix's exact bit pattern, so it stays correct even if someone refactors the math.

## Scope

This is a small, focused utility. Keep it minimal:

- Open one or more single-page PDFs
- Preview each page
- Place a stamp (image or text) at a user-chosen position via click/drag
- Batch-apply the same stamp to all loaded PDFs
- Export/save without quality loss (no re-encoding raster content)

**Out of scope**: multi-page editing, text editing, annotations, form filling, OCR.

## Key Constraints

- Stamps are overlaid via Form XObject — **never re-encode existing page content**
- Stamp positioning uses the PDF `cm` (concat matrix) operator for images, `Td` for text
- Keep the UI dead simple: file list, preview pane, stamp controls
- Target single-binary distribution via Tauri
- pdfium library is dynamically loaded from `src-tauri/libs/pdfium/lib` (dev) or next to the executable (prod)

## Known Issues / Active Bugs

- **Image stamp 180° flip + offset on non-rotated pages, varied breakage on `/Rotate` pages** *(fixed in 1622433)*: `image_cm_for_rotation` had the wrong matrix in all four rotation branches. Fixed and covered by property-based tests.
- **Text stamp coordinate transform on `/Rotate` pages** *(fixed in a5410f9)*: `coord_cm_for_rotation` had its 90° and 270° branches swapped. Fixed and covered by round-trip property tests.
- **Stamp inherits polluted CTM from the existing page Contents** *(fixed in e633776)*: many PDFs leave a non-identity CTM at end of stream; we now wrap existing Contents in `q ... Q` before appending the stamp. Covered by `stamp_image_survives_polluted_ctm_in_existing_contents`.
- **Transparent PNG renders with white background**: `create_image_xobject()` calls `to_rgb8()` and drops the alpha channel. Fix requires emitting an `SMask` (single-channel DeviceGray Image XObject built from the alpha channel) on the main image XObject.
- **Output filename collisions silently overwrite**: Two input PDFs with the same filename in different directories produce identical output paths in `stamp_pdfs`. No conflict detection.
- **Batch progress bar lies**: `stamp_pdfs` runs to completion before returning; the UI's `setExportProgress` only fires once at the end. Need `Channel<T>` or `app.emit` per-file events.
- **Inherited MediaBox** *(handled)*: Some PDFs inherit `MediaBox` from parent `Pages` nodes. `get_page_geometry()` walks up the page tree to resolve it.

## Build & Run

```bash
# Dev (starts both Vite frontend and Rust backend)
npm run tauri dev

# Production build
npm run tauri build

# Rust type-check only (fast)
cd src-tauri && cargo check

# Rust tests
cd src-tauri && cargo test

# TypeScript type-check
npx tsc --noEmit

# Frontend tests
npx vitest
```

## Conventions

- Rust: snake_case, `Result<T, E>` for errors, `thiserror` crate, `///` doc comments on pub items only
- TypeScript: strict mode, no `any`, camelCase vars, PascalCase components, one component per file
- Files: kebab-case
- State: Zustand stores in `src/stores/`, IPC wrappers in `src/services/`
- Commits: `type(scope): message`
- Tauri commands should be thin — delegate logic to `pdf.rs`
- No `console.log` in production code
