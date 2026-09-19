import type { ReactNode } from 'react';

type Tone = 'neutral' | 'warn';

const toneStyles: Record<Tone, React.CSSProperties> = {
  neutral: {
    background: 'var(--color-bg)',
    border: '1px solid var(--color-border)',
    color: 'var(--color-text-muted)',
  },
  warn: {
    background: 'rgba(217,154,61,0.16)',
    border: '1px solid var(--color-warn)',
    color: 'var(--color-warn)',
  },
};

export function Badge({ tone = 'neutral', children }: { tone?: Tone; children: ReactNode }) {
  return (
    <span
      style={{
        fontSize: 11,
        padding: '2px 9px',
        borderRadius: 'var(--radius-pill)',
        fontWeight: 500,
        ...toneStyles[tone],
      }}
    >
      {children}
    </span>
  );
}
