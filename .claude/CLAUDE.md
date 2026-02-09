# PDF Stamper

Batch tool for adding stamps (images/text) to single-page PDF files without quality loss.

## Tech Stack

- **Framework**: Tauri v2 (Rust backend + web frontend)
- **Frontend**: React + TypeScript + Vite
- **PDF Rendering**: pdfium via `pdfium-render` crate
- **PDF Manipulation**: lopdf (Rust, MIT license)
- **Styling**: Tailwind CSS

## Project Structure

```
src-tauri/src/
  commands.rs       — Tauri IPC handlers
  pdf.rs            — PDF open/save/stamp logic
  lib.rs            — App setup

src/
  components/       — React components
  stores/           — Zustand state
  services/         — Tauri IPC bridge
  App.tsx
  main.tsx
```

## Scope

This is a small, focused utility. Keep it minimal:

- Open one or more single-page PDFs
- Preview each page
- Place a stamp (image or text) at a user-chosen position via click/drag
- Batch-apply the same stamp to all loaded PDFs
- Export/save without quality loss (no re-encoding raster content)

**Out of scope**: multi-page editing, text editing, annotations, form filling, OCR.

## Key Constraints

- Stamps are overlaid — never re-encode existing page content
- Use incremental save to preserve original quality
- Keep the UI dead simple: file list, preview pane, stamp controls
- Target single-binary distribution via Tauri

## Build & Run

```bash
# Dev
npm run tauri dev

# Build
npm run tauri build

# Rust tests
cd src-tauri && cargo test

# Frontend tests
npx vitest
```

## Conventions

- Rust: snake_case, `Result<T, E>` for errors, `thiserror` crate
- TypeScript: strict mode, no `any`, camelCase vars, PascalCase components
- Files: kebab-case
- Commits: `type(scope): message` (see global git-workflow rule)
