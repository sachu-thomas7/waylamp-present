import { useEffect, useState } from 'react';
import { useSlideStore } from '../store/slideStore';

function computeScale(containerWidth: number, containerHeight: number): {
  scale: number;
  marginLeft: number;
  marginTop: number;
} {
  const scale = Math.min(containerWidth / 1920, containerHeight / 1080);
  const marginLeft = (containerWidth - 1920 * scale) / 2;
  const marginTop = (containerHeight - 1080 * scale) / 2;
  return { scale, marginLeft, marginTop };
}

export function SlideCanvas() {
  const slide = useSlideStore((s) => s.current);
  const [layout, setLayout] = useState(() =>
    computeScale(window.innerWidth, window.innerHeight)
  );

  useEffect(() => {
    const observer = new ResizeObserver(() => {
      setLayout(computeScale(window.innerWidth, window.innerHeight));
    });
    observer.observe(document.body);
    return () => observer.disconnect();
  }, []);

  const { scale, marginLeft, marginTop } = layout;

  return (
    <div
      style={{
        width: '100vw',
        height: '100vh',
        background: '#000',
        overflow: 'hidden',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
      }}
    >
      <div
        style={{
          width: 1920,
          height: 1080,
          position: 'relative',
          transform: `scale(${scale})`,
          transformOrigin: 'top left',
          marginLeft,
          marginTop,
          background: slide?.background_path ? undefined : '#1a1a2e',
        }}
      >
        {/* Background zone */}
        {slide?.background_path && (
          <img
            src={slide.background_path}
            alt=""
            style={{
              position: 'absolute',
              inset: 0,
              width: '100%',
              height: '100%',
              objectFit: 'cover',
              zIndex: 0,
            }}
          />
        )}

        {/* Title zone */}
        <div
          style={{
            position: 'absolute',
            top: '10%',
            left: '5%',
            width: '90%',
            zIndex: 1,
            color: '#fff',
            fontSize: 72,
            fontWeight: 700,
            fontFamily: 'system-ui, sans-serif',
            lineHeight: 1.2,
          }}
        >
          {slide?.title ?? ''}
        </div>

        {/* Body zone */}
        <div
          style={{
            position: 'absolute',
            top: '30%',
            left: '5%',
            width: '90%',
            height: '55%',
            zIndex: 1,
            color: '#e0e0e0',
            fontSize: 40,
            fontFamily: 'system-ui, sans-serif',
            lineHeight: 1.5,
            overflowY: 'hidden',
          }}
        >
          {slide?.body ?? ''}
        </div>

        {/* Slide number */}
        <div
          style={{
            position: 'absolute',
            bottom: '3%',
            right: '3%',
            zIndex: 2,
            color: 'rgba(255,255,255,0.5)',
            fontSize: 28,
            fontFamily: 'system-ui, sans-serif',
          }}
        >
          {slide ? `${slide.index + 1} / ${slide.total}` : ''}
        </div>
      </div>
    </div>
  );
}