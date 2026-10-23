import { useEffect, useState } from 'react';
import { getStartupNotices } from '../lib/ipc';
import { Button } from './ui/Button';

export function StartupNotices() {
  const [notices, setNotices] = useState<string[]>([]);

  useEffect(() => {
    getStartupNotices().then(setNotices).catch(() => undefined);
  }, []);

  if (notices.length === 0) return null;

  return (
    <div
      role="alert"
      style={{
        flexShrink: 0, display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 16,
        padding: '10px 24px', background: 'var(--color-surface)', borderBottom: '1px solid var(--color-warn)',
        color: 'var(--color-warn)', fontSize: 13,
      }}
    >
      <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
        {notices.map((n) => (
          <span key={n}>{n}</span>
        ))}
      </div>
      <Button onClick={() => setNotices([])}>Dismiss</Button>
    </div>
  );
}
