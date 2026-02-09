/**
 * Convert screen pixel coordinates to PDF point coordinates.
 * PDF origin is bottom-left; screen origin is top-left.
 */
export function screenToPdf(
  screenX: number,
  screenY: number,
  imageWidth: number,
  imageHeight: number,
  pageWidthPt: number,
  pageHeightPt: number,
): { x: number; y: number } {
  const x = (screenX / imageWidth) * pageWidthPt;
  const y = pageHeightPt - (screenY / imageHeight) * pageHeightPt;
  return { x, y };
}

/**
 * Convert PDF point coordinates to screen pixel coordinates.
 */
export function pdfToScreen(
  pdfX: number,
  pdfY: number,
  imageWidth: number,
  imageHeight: number,
  pageWidthPt: number,
  pageHeightPt: number,
): { x: number; y: number } {
  const x = (pdfX / pageWidthPt) * imageWidth;
  const y = ((pageHeightPt - pdfY) / pageHeightPt) * imageHeight;
  return { x, y };
}

/**
 * Convert a size in PDF points to screen pixels.
 */
export function pdfSizeToScreen(
  widthPt: number,
  heightPt: number,
  imageWidth: number,
  imageHeight: number,
  pageWidthPt: number,
  pageHeightPt: number,
): { width: number; height: number } {
  const width = (widthPt / pageWidthPt) * imageWidth;
  const height = (heightPt / pageHeightPt) * imageHeight;
  return { width, height };
}
