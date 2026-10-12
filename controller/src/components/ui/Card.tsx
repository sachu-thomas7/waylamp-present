import type { ReactNode } from 'react';

interface CardProps {
  title: string;
  headerRight?: ReactNode;
  children: ReactNode;
  /** Grow to fill the remaining height of a flex-column parent. */
  fill?: boolean;
}

export function Card({ title, headerRight, children, fill = false }: CardProps) {
  return (
    <div
      style={{
        background: 'var(--color-surface)',
        border: '1px solid var(--color-border)',
        borderRadius: 12,
        padding: 18,
        display: 'flex',
        flexDirection: 'column',
        gap: 14,
        ...(fill ? { flex: 1, minHeight: 0 } : {}),
      }}
    >
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 12 }}>
        <span
          style={{
            fontSize: 11.5,
            fontWeight: 600,
            letterSpacing: '0.07em',
            textTransform: 'uppercase',
            color: 'var(--color-text-muted)',
          }}
        >
          {title}
        </span>
        {headerRight}
      </div>
      {children}
    </div>
  );
}
