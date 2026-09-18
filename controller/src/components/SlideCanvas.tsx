import type { SlidePayload } from '../lib/ipc';

interface SlideCanvasProps {
  slide: SlidePayload;
  scale?: number;
}

export function SlideCanvas({ slide, scale = 1.0 }: SlideCanvasProps) {
  const containerStyle: React.CSSProperties = {
    width: `${Math.round(1920 * scale)}px`,
    height: `${Math.round(1080 * scale)}px`,
    overflow: 'hidden',
    flexShrink: 0,
  };

  const innerStyle: React.CSSProperties = {
    width: '1920px',
    height: '1080px',
    transform: `scale(${scale})`,
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
    <div style={containerStyle}>
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