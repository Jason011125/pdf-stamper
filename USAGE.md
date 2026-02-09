# PDF Stamper — Usage Guide

A desktop tool for batch-adding image or text stamps to single-page PDFs without quality loss.

## Getting Started

### Dev mode
```bash
npm run tauri dev
```

### Production build
```bash
npm run tauri build
```

## Workflow

### 1. Open PDFs

Click **Open PDFs** in the left sidebar. Select one or more PDF files from the native file picker. They appear in the file list — click any filename to preview it in the center pane.

To add more files later, click **Open PDFs** again. New files are appended to the existing list.

To remove a file, click the **×** button next to its name.

### 2. Configure the Stamp

Use the right panel to set up your stamp.

**Image stamp:**
- Select the **Image** tab
- Click **Upload Image** and pick a PNG, JPG, or WebP file
- The image preview appears below the button
- Click the red **×** on the preview to clear it

**Text stamp:**
- Select the **Text** tab
- Type your stamp text
- Adjust **Size** (font size in points) and **Color**

**Stamp dimensions:**
- Set **W** (width) and **H** (height) in PDF points (1 point = 1/72 inch)
- These control the size of the stamp overlay on the page

### 3. Place the Stamp

Click anywhere on the PDF preview to place the stamp. It appears as a blue dashed rectangle with your image or text inside.

- **Click** to place or reposition
- **Drag** the stamp rectangle to fine-tune its position
- The position coordinates (in PDF points) are shown below the size inputs

### 4. Export

Click **Apply to All** at the bottom of the right panel. A native folder picker opens — select the output directory.

Each PDF is saved as `<original-name>-stamped.pdf` in the chosen folder. The original files are never modified.

Progress is shown on the button during export.

## Tips

- Stamps are overlaid on the PDF — original content is never re-encoded or degraded
- The stamp position and size are the same across all files in the batch
- For best results, use PNG images with a transparent background
- PDF points: A4 is 595 × 842 pt, US Letter is 612 × 792 pt
