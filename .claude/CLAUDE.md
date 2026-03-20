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

### Page Rotation
PDF pages may have a `/Rotate` entry (0, 90, 180, 270 degrees CW). The `get_page_geometry()` function handles this and returns both raw and effective (as-displayed) dimensions. pdfium renders with rotation applied, so the preview matches viewer display.

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

- **Stamp position/orientation mismatch on rotated pages**: Pages with `/Rotate` cause the stamp to appear at the wrong position and potentially flipped in the saved PDF. The fix requires transforming stamp coordinates based on page rotation in `stamp_image()` and `stamp_text()`.
- **Inherited MediaBox**: Some PDFs inherit MediaBox from parent Pages nodes. `get_page_geometry()` now handles this by walking up the page tree.

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
