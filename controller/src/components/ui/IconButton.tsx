import type { ButtonHTMLAttributes, ReactNode } from 'react';

interface IconButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  // Required, not optional — every icon-only control needs a stated label (Apple-simplicity review).
  title: string;
  children: ReactNode;
  active?: boolean;
}

export function IconButton({ children, active, style, disabled, ...rest }: IconButtonProps) {
  return (
    <button
      disabled={disabled}
      style={{
        width: 30,
        height: 30,
        borderRadius: 'var(--radius-sm)',
        background: active ? 'var(--color-primary)' : 'transparent',
        border: active ? '1px solid var(--color-primary)' : '1px solid var(--color-border)',
        color: active ? 'var(--color-primary-ink)' : 'var(--color-text)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        cursor: disabled ? 'not-allowed' : 'pointer',
        opacity: disabled ? 0.45 : 1,
        ...style,
      }}
      {...rest}
    >
      {children}
    </button>
  );
}
