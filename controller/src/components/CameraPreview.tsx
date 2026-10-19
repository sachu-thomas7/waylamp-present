import { usePreviewStore } from '../store/previewStore';

const overlay = {
  position: 'absolute', left: 0, right: 0, bottom: 0, padding: '14px 8px 5px',
  background: 'linear-gradient(transparent, rgba(0,0,0,0.75))', color: '#fff',
  display: 'flex', flexDirection: 'column', gap: 1, pointerEvents: 'none',
} as const;
const center = {
  position: 'absolute', inset: 0, display: 'flex', alignItems: 'center', justifyContent: 'center',
  fontSize: 12, color: 'var(--color-text-muted)',
} as const;

export function CameraPreview({ name, lost }: { name: string; lost: boolean }) {
  const frame = usePreviewStore((s) => s.frames[name]);
  const stats = usePreviewStore((s) => s.stats[name]);

  return (
    <div
      data-testid={`camera-preview-${name}`}
      style={{
        position: 'relative', aspectRatio: '16 / 9', background: 'var(--color-bg)',
        border: `1px solid ${lost ? 'var(--color-warn)' : 'var(--color-border)'}`,
        borderRadius: 'var(--radius-sm)', overflow: 'hidden',
      }}
    >
      {lost ? (
        <div style={{ ...center, color: 'var(--color-warn)', fontWeight: 600 }} data-testid={`preview-lost-${name}`}>
          Source Lost
        </div>
      ) : frame ? (
        <img
          data-testid={`preview-image-${name}`}
          src={`data:image/png;base64,${frame.png_base64}`}
          alt={`${name} preview`}
          style={{ width: '100%', height: '100%', objectFit: 'cover', display: 'block' }}
        />
      ) : (
        <div style={center}>Waiting for video…</div>
      )}
      <div style={overlay}>
        <span style={{ fontSize: 12, fontWeight: 600, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{name}</span>
        <span data-testid={`preview-meta-${name}`} style={{ fontFamily: 'var(--font-mono)', fontSize: 10.5, opacity: 0.85 }}>
          {stats && !lost ? `${stats.width}×${stats.height} · ${stats.fps.toFixed(1)} fps · ${stats.latency_ms} ms` : '—'}
        </span>
      </div>
    </div>
  );
}
