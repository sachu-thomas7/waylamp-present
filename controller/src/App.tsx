// controller/src/App.tsx
import { useEffect, useState } from 'react';
import { getBuildInfo, deckClear } from './lib/ipc';
import { PresentButton } from './components/PresentButton';
import { SlideDeckPanel } from './components/SlideDeckPanel';
import { SceneMediaPanel } from './components/SceneMediaPanel';
import { DisplayPanel } from './components/DisplayPanel';
import { PipelinePanel } from './components/PipelinePanel';
import { NdiSourcesPanel } from './components/NdiSourcesPanel';
import { Popover } from './components/ui/Popover';
import { IconButton } from './components/ui/IconButton';
import { Button } from './components/ui/Button';
import { IconHelp, IconPlay, IconSettings } from './components/ui/icons';

interface BuildInfo {
  build_timestamp: string;
  version: string;
}

const SHORTCUTS: Array<[string, string]> = [
  ['Space / →', 'Advance slide'],
  ['←', 'Rewind slide'],
  ['Esc', 'Exit presentation'],
];

export default function App() {
  const [buildInfo, setBuildInfo] = useState<BuildInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    getBuildInfo()
      .then(setBuildInfo)
      .catch((e: unknown) => setError(String(e)));
  }, []);

  return (
    <div style={{ height: '100%', display: 'flex', flexDirection: 'column' }}>
      {/* Top bar */}
      <div style={{
        flexShrink: 0, height: 64, background: 'var(--color-surface)', borderBottom: '1px solid var(--color-border)',
        display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '0 24px',
      }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 11 }}>
          <div style={{ width: 32, height: 32, borderRadius: 8, background: 'var(--color-primary)', display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0, color: 'var(--color-primary-ink)' }}>
            <IconPlay />
          </div>
          <div style={{ display: 'flex', flexDirection: 'column', lineHeight: 1.2 }}>
            <span style={{ fontSize: 15, fontWeight: 600, letterSpacing: '0.01em' }}>StreamDeck</span>
            <span style={{ fontSize: 11, color: 'var(--color-text-muted)' }}>Controller</span>
          </div>
        </div>

        <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
          <Popover
            width={220}
            trigger={({ onClick }) => (
              <IconButton title="Keyboard shortcuts" onClick={onClick}>
                <IconHelp />
              </IconButton>
            )}
          >
            <span style={{ fontSize: 11.5, fontWeight: 600, letterSpacing: '0.07em', textTransform: 'uppercase', color: 'var(--color-text-muted)' }}>
              Shortcuts
            </span>
            {SHORTCUTS.map(([key, label]) => (
              <div key={key} style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                <span style={{ fontFamily: 'var(--font-mono)', fontSize: 11, padding: '3px 8px', borderRadius: 5, background: 'var(--color-bg)', border: '1px solid var(--color-border)' }}>{key}</span>
                <span style={{ fontSize: 12, color: 'var(--color-text-muted)' }}>{label}</span>
              </div>
            ))}
          </Popover>

          <Popover
            width={300}
            trigger={({ onClick }) => (
              <IconButton title="Settings — displays, build info" onClick={onClick}>
                <IconSettings />
              </IconButton>
            )}
          >
            <DisplayPanel />
            <div style={{ borderTop: '1px solid var(--color-border)', paddingTop: 10 }}>
              <PipelinePanel />
            </div>
            <div style={{ borderTop: '1px solid var(--color-border)', paddingTop: 10 }}>
              <NdiSourcesPanel />
            </div>
            <div style={{ borderTop: '1px solid var(--color-border)', paddingTop: 10, display: 'flex', flexDirection: 'column', gap: 8 }}>
              <span style={{ fontSize: 11.5, fontWeight: 600, letterSpacing: '0.07em', textTransform: 'uppercase', color: 'var(--color-text-muted)' }}>Deck</span>
              <Button
                variant="secondary"
                onClick={() => deckClear().catch(console.error)}
                title="Clears the loaded deck for this session — imported files aren't deleted, just unloaded"
              >
                Clear Deck
              </Button>
            </div>
            <div style={{ borderTop: '1px solid var(--color-border)', paddingTop: 10, display: 'flex', flexDirection: 'column', gap: 3 }}>
              <span style={{ fontSize: 11.5, fontWeight: 600, letterSpacing: '0.07em', textTransform: 'uppercase', color: 'var(--color-text-muted)' }}>Build Info</span>
              {error && <span style={{ fontSize: 11.5, color: 'var(--color-warn)' }}>IPC error: {error}</span>}
              {buildInfo ? (
                <>
                  <span style={{ fontSize: 11.5, color: 'var(--color-text-muted)' }}>Timestamp {buildInfo.build_timestamp}</span>
                  <span style={{ fontSize: 11.5, color: 'var(--color-text-muted)' }}>Version v{buildInfo.version}</span>
                </>
              ) : (
                <span style={{ fontSize: 11.5, color: 'var(--color-text-muted)' }}>Loading…</span>
              )}
            </div>
          </Popover>

          <PresentButton />
        </div>
      </div>

      {/* Main content */}
      <div style={{ flex: 1, display: 'flex', gap: 20, padding: '20px 24px', minHeight: 0, overflow: 'auto' }}>
        <div style={{ flex: '1 1 68%', display: 'flex', flexDirection: 'column', gap: 16, minWidth: 0 }}>
          <SlideDeckPanel />
        </div>
        <div style={{ flex: '1 1 32%', maxWidth: 340, display: 'flex', flexDirection: 'column', gap: 16 }}>
          <SceneMediaPanel />
        </div>
      </div>
    </div>
  );
}
