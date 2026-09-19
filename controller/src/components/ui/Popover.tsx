import { useState, type ReactNode } from 'react';

interface PopoverProps {
  trigger: (props: { onClick: () => void }) => ReactNode;
  children: ReactNode;
  width?: number;
}

export function Popover({ trigger, children, width = 280 }: PopoverProps) {
  const [open, setOpen] = useState(false);

  return (
    <div style={{ position: 'relative' }}>
      {trigger({ onClick: () => setOpen((o) => !o) })}
      {open && (
        <>
          <div
            onClick={() => setOpen(false)}
            style={{ position: 'fixed', inset: 0, zIndex: 10 }}
          />
          <div
            style={{
              position: 'absolute',
              top: '100%',
              right: 0,
              marginTop: 8,
              width,
              background: 'var(--color-surface)',
              border: '1px solid var(--color-border)',
              borderRadius: 12,
              padding: 16,
              boxShadow: '0 8px 24px rgba(0,0,0,0.4)',
              zIndex: 11,
              display: 'flex',
              flexDirection: 'column',
              gap: 12,
            }}
          >
            {children}
          </div>
        </>
      )}
    </div>
  );
}
