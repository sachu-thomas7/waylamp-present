interface Option<T extends string> {
  value: T;
  label: string;
}

interface SegmentedControlProps<T extends string> {
  options: Option<T>[];
  value: T;
  onChange: (value: T) => void;
}

export function SegmentedControl<T extends string>({ options, value, onChange }: SegmentedControlProps<T>) {
  return (
    <div
      role="group"
      style={{
        display: 'inline-flex',
        background: 'var(--color-bg)',
        border: '1px solid var(--color-border)',
        borderRadius: 'var(--radius-pill)',
        padding: 3,
        alignSelf: 'flex-start',
      }}
    >
      {options.map((opt) => {
        const selected = opt.value === value;
        return (
          <button
            key={opt.value}
            type="button"
            aria-pressed={selected}
            onClick={() => onChange(opt.value)}
            style={{
              padding: '6px 14px',
              borderRadius: 'var(--radius-pill)',
              border: 'none',
              background: selected ? 'var(--color-primary)' : 'transparent',
              color: selected ? 'var(--color-primary-ink)' : 'var(--color-text-muted)',
              fontSize: 12,
              fontWeight: selected ? 600 : 500,
              fontFamily: 'var(--font-body)',
              cursor: 'pointer',
            }}
          >
            {opt.label}
          </button>
        );
      })}
    </div>
  );
}
