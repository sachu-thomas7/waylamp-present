import type { ButtonHTMLAttributes, ReactNode } from 'react';

type Variant = 'primary' | 'live' | 'secondary';

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  icon?: ReactNode;
  pill?: boolean;
}

const variantStyles: Record<Variant, React.CSSProperties> = {
  primary: {
    background: 'var(--color-primary)',
    color: 'var(--color-primary-ink)',
    border: 'none',
  },
  live: {
    background: 'var(--color-live)',
    color: 'var(--color-live-ink)',
    border: 'none',
  },
  secondary: {
    background: 'transparent',
    color: 'var(--color-text)',
    border: '1px solid var(--color-border)',
  },
};

export function Button({ variant = 'secondary', icon, pill, children, style, disabled, ...rest }: ButtonProps) {
  return (
    <button
      disabled={disabled}
      style={{
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        gap: 7,
        padding: pill ? '10px 20px' : '7px 13px',
        borderRadius: pill ? 'var(--radius-pill)' : 'var(--radius-sm)',
        fontFamily: 'var(--font-body)',
        fontSize: pill ? 14 : 12.5,
        fontWeight: pill ? 600 : 500,
        cursor: disabled ? 'not-allowed' : 'pointer',
        opacity: disabled ? 0.45 : 1,
        ...variantStyles[variant],
        ...style,
      }}
      {...rest}
    >
      {icon}
      {children}
    </button>
  );
}
