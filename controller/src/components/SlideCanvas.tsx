import { useEffect, useRef, useState } from 'react';
import type { SlidePayload } from '../lib/ipc';

interface SlideCanvasProps {
  slide: SlidePayload;
  scale?: number;
  /** Fill the parent's width at 16:9, ignoring `scale`. */
  fluid?: boolean;
}

export function SlideCanvas({ slide, scale = 1.0, fluid = false }: SlideCanvasProps) {
  const ref = useRef<HTMLDivElement>(null);
  const [fluidWidth, setFluidWidth] = useState(0);

  useEffect(() => {
    const el = ref.current;
    if (!fluid || !el || typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(([entry]) => setFluidWidth(entry.contentRect.width));
    observer.observe(el);
    return () => observer.disconnect();
  }, [fluid]);

  const effectiveScale = fluid ? fluidWidth / 1920 : scale;

  const containerStyle: React.CSSProperties = fluid
    ? { width: '100%', aspectRatio: '16 / 9', overflow: 'hidden' }
    : {
        width: `${Math.round(1920 * scale)}px`,
        height: `${Math.round(1080 * scale)}px`,
        overflow: 'hidden',
        flexShrink: 0,
      };

  const innerStyle: React.CSSProperties = {
    width: '1920px',
    height: '1080px',
    transform: `scale(${effectiveScale})`,
    transformOrigin: 'top left',
    backgroundColor: slide.background_path ? undefined : '#1a1a2e',
    backgroundImage: slide.background_path
      ? `url(${slide.background_path})`
      : undefined,
    backgroundSize: 'cover',
    position: 'relative',
    boxSizing: 'border-box',
  };

  return (
    <div ref={ref} style={containerStyle}>
      <div style={innerStyle}>
        <div
          style={{
            position: 'absolute', top: 80, left: 80, right: 80,
            color: '#fff', fontSize: 72, fontWeight: 'bold', lineHeight: 1.1,
          }}
        >
          {slide.title}
        </div>
        <div
          style={{
            position: 'absolute', top: 260, left: 80, right: 80,
            color: '#fff', fontSize: 40, lineHeight: 1.4,
          }}
        >
          {slide.body}
        </div>
      </div>
    </div>
  );
}
