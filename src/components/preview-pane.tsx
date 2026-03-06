import { useRef, useCallback, useState, useEffect } from 'react';
import { usePdfStore } from '../stores/pdf-store';
import { useStampStore } from '../stores/stamp-store';
import { pdfSizeToScreen } from '../services/coordinate-utils';

export function PreviewPane(): React.JSX.Element {
  const files = usePdfStore((s) => s.files);
  const selectedIndex = usePdfStore((s) => s.selectedIndex);
  const setStampPos = usePdfStore((s) => s.setStampPos);
  const file = files[selectedIndex];

  const stampType = useStampStore((s) => s.type);
  const imagePreviewUrl = useStampStore((s) => s.imagePreviewUrl);
  const text = useStampStore((s) => s.text);
  const fontSize = useStampStore((s) => s.fontSize);
  const color = useStampStore((s) => s.color);
  const globalXPt = useStampStore((s) => s.xPt);
  const globalYPt = useStampStore((s) => s.yPt);
  const widthPt = useStampStore((s) => s.widthPt);
  const heightPt = useStampStore((s) => s.heightPt);
  const rotationDeg = useStampStore((s) => s.rotationDeg);
  const isPlaced = useStampStore((s) => s.isPlaced);
  const setPosition = useStampStore((s) => s.setPosition);
  const setPlaced = useStampStore((s) => s.setPlaced);

  // Effective position: use this file's individual position if set, else global
  const xPt = file?.stampPos?.xPt ?? globalXPt;
  const yPt = file?.stampPos?.yPt ?? globalYPt;
  const isStampVisible = isPlaced || (file?.stampPos != null);

  const containerRef = useRef<HTMLDivElement>(null);
  const imageRef = useRef<HTMLImageElement>(null);
  const [imageSize, setImageSize] = useState({ width: 0, height: 0 });
  const didDragRef = useRef(false);

  // Track image display size
  useEffect(() => {
    if (!imageRef.current) return;
    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) {
        setImageSize({
          width: entry.contentRect.width,
          height: entry.contentRect.height,
        });
      }
    });
    observer.observe(imageRef.current);
    return () => observer.disconnect();
  }, [file?.previewUrl]);

  // Convert stored PDF-point position to screen pixels for display
  const toScreenPos = useCallback(
    (pdfX: number, pdfY: number): { x: number; y: number } => {
      if (!file || imageSize.width === 0) return { x: 0, y: 0 };
      const x = (pdfX / file.widthPt) * imageSize.width;
      // PDF y is from bottom; screen y is from top.
      const y =
        imageSize.height -
        ((pdfY + heightPt) / file.heightPt) * imageSize.height;
      return { x, y };
    },
    [file, imageSize, heightPt],
  );

  // Convert screen top-left pixel position to PDF bottom-left point position
  const toPdfPos = useCallback(
    (screenX: number, screenY: number): { x: number; y: number } => {
      if (!file || imageSize.width === 0) return { x: 0, y: 0 };
      const x = (screenX / imageSize.width) * file.widthPt;
      const stampScreenH = (heightPt / file.heightPt) * imageSize.height;
      const bottomScreenY = screenY + stampScreenH;
      const y =
        file.heightPt - (bottomScreenY / imageSize.height) * file.heightPt;
      return { x, y };
    },
    [file, imageSize, heightPt],
  );

  // Write position: update both the current file's individual position AND the global
  const applyPosition = useCallback(
    (pdfX: number, pdfY: number) => {
      setStampPos(selectedIndex, pdfX, pdfY); // lock this file's position
      setPosition(pdfX, pdfY);               // advance global (unset files inherit)
      setPlaced(true);
    },
    [selectedIndex, setStampPos, setPosition, setPlaced],
  );

  // Click on preview to place stamp
  const handleImageClick = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      if (didDragRef.current) {
        didDragRef.current = false;
        return;
      }
      if (!file || imageSize.width === 0) return;

      const rect = e.currentTarget.getBoundingClientRect();
      const clickX = e.clientX - rect.left;
      const clickY = e.clientY - rect.top;

      const stampScreenSize = pdfSizeToScreen(
        widthPt,
        heightPt,
        imageSize.width,
        imageSize.height,
        file.widthPt,
        file.heightPt,
      );
      const topLeftX = Math.max(
        0,
        Math.min(clickX - stampScreenSize.width / 2, imageSize.width - stampScreenSize.width),
      );
      const topLeftY = Math.max(
        0,
        Math.min(clickY - stampScreenSize.height / 2, imageSize.height - stampScreenSize.height),
      );

      const pdf = toPdfPos(topLeftX, topLeftY);
      applyPosition(pdf.x, pdf.y);
    },
    [file, imageSize, widthPt, heightPt, toPdfPos, applyPosition],
  );

  // Drag stamp to reposition
  const handleStampMouseDown = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (!file || imageSize.width === 0) return;

      const stampEl = e.currentTarget as HTMLElement;
      const stampRect = stampEl.getBoundingClientRect();
      const offsetX = e.clientX - stampRect.left;
      const offsetY = e.clientY - stampRect.top;

      didDragRef.current = false;

      const handleMove = (me: MouseEvent): void => {
        didDragRef.current = true;
        const parentRect = containerRef.current?.getBoundingClientRect();
        if (!parentRect) return;

        const stampScreenSize = pdfSizeToScreen(
          widthPt,
          heightPt,
          imageSize.width,
          imageSize.height,
          file.widthPt,
          file.heightPt,
        );

        const newLeft = me.clientX - parentRect.left - offsetX;
        const newTop = me.clientY - parentRect.top - offsetY;

        const clampedX = Math.max(0, Math.min(newLeft, imageSize.width - stampScreenSize.width));
        const clampedY = Math.max(0, Math.min(newTop, imageSize.height - stampScreenSize.height));

        const pdf = toPdfPos(clampedX, clampedY);
        applyPosition(pdf.x, pdf.y);
      };

      const handleUp = (): void => {
        window.removeEventListener('mousemove', handleMove);
        window.removeEventListener('mouseup', handleUp);
      };

      window.addEventListener('mousemove', handleMove);
      window.addEventListener('mouseup', handleUp);
    },
    [file, imageSize, widthPt, heightPt, toPdfPos, applyPosition],
  );

  if (!file) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <p className="text-gray-400">Open PDF files to get started</p>
      </div>
    );
  }

  if (!file.previewUrl) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="flex items-center gap-2 text-gray-400">
          <svg className="animate-spin h-5 w-5" viewBox="0 0 24 24">
            <circle
              className="opacity-25"
              cx="12"
              cy="12"
              r="10"
              stroke="currentColor"
              strokeWidth="4"
              fill="none"
            />
            <path
              className="opacity-75"
              fill="currentColor"
              d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
            />
          </svg>
          Rendering...
        </div>
      </div>
    );
  }

  const screenPos = toScreenPos(xPt, yPt);
  const stampScreenSize =
    file && imageSize.width > 0
      ? pdfSizeToScreen(
          widthPt,
          heightPt,
          imageSize.width,
          imageSize.height,
          file.widthPt,
          file.heightPt,
        )
      : { width: 0, height: 0 };

  return (
    <div className="flex-1 flex items-center justify-center p-4 overflow-auto bg-gray-100">
      <div ref={containerRef} className="relative inline-block" onClick={handleImageClick}>
        <img
          ref={imageRef}
          src={file.previewUrl}
          alt={file.filename}
          className="max-h-full shadow-lg"
          draggable={false}
          onLoad={(e) => {
            const img = e.currentTarget;
            setImageSize({ width: img.clientWidth, height: img.clientHeight });
          }}
        />

        {isStampVisible && imageSize.width > 0 && (
          <div
            onMouseDown={handleStampMouseDown}
            className="absolute border-2 border-dashed border-blue-500 bg-blue-500/10 cursor-move flex items-center justify-center overflow-hidden"
            style={{
              left: screenPos.x,
              top: screenPos.y,
              width: stampScreenSize.width,
              height: stampScreenSize.height,
              transform: `rotate(${rotationDeg}deg)`,
              transformOrigin: 'center center',
            }}
          >
            {stampType === 'image' && imagePreviewUrl && (
              <img
                src={imagePreviewUrl}
                alt="stamp"
                className="w-full h-full object-contain pointer-events-none"
                draggable={false}
              />
            )}
            {stampType === 'text' && (
              <span
                className="pointer-events-none whitespace-nowrap"
                style={{
                  fontSize: `${(fontSize / file.heightPt) * imageSize.height}px`,
                  color,
                }}
              >
                {text}
              </span>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
