import { useEffect } from 'react';
import { useNdiStore } from '../store/ndiStore';
import { onAppEvent } from '../lib/events';
import { ndiAddSource, ndiListSources, ndiRemoveSource } from '../lib/ipc';
import { Badge } from './ui/Badge';
import { Button } from './ui/Button';

const heading = { fontSize: 11.5, fontWeight: 600, letterSpacing: '0.07em', textTransform: 'uppercase', color: 'var(--color-text-muted)' } as const;
const row = { display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 8 } as const;
const note = { fontSize: 11.5, margin: 0 } as const;

export function NdiSourcesPanel() {
  const { discovered, added, unavailable, lastError, setSnapshot, setDiscovered, markAdded, markRemoved, setLost, setLastError } =
    useNdiStore();

  useEffect(() => {
    ndiListSources().then(setSnapshot).catch(console.error);

    const unlisten = onAppEvent((event) => {
      switch (event.type) {
        case 'SourceListChanged':
          setDiscovered(event.payload.sources);
          break;
        case 'SourceLost':
          setLost(event.payload.name, true);
          break;
        case 'SourceDiscovered':
          setLost(event.payload.source.name, false);
          break;
        case 'SourceAdded':
          markAdded(event.payload.name, event.payload.format);
          break;
        case 'SourceRemoved':
          markRemoved(event.payload.name);
          break;
      }
    });

    return () => { unlisten.then((fn) => fn()); };
  }, []);

  async function run(action: () => Promise<void>) {
    setLastError(null);
    try {
      await action();
    } catch (e) {
      setLastError(JSON.stringify(e));
    }
  }

  const available = discovered.filter((d) => !added.some((a) => a.name === d.name));

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
      <span style={heading}>NDI Sources</span>
      {unavailable && <p style={{ ...note, color: 'var(--color-warn)' }}>{unavailable}</p>}
      {lastError && <p style={{ ...note, color: 'var(--color-warn)' }}>{lastError}</p>}
      {added.map((a) => (
        <div key={a.name} style={row} data-testid={`ndi-added-${a.name}`}>
          <span style={{ fontSize: 12.5 }}>{a.name}</span>
          <span style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
            {a.lost ? <Badge tone="warn">Source Lost</Badge> : <Badge>{a.format === 'NewFormat' ? 'New Format' : 'Standard'}</Badge>}
            <Button variant="secondary" onClick={() => run(() => ndiRemoveSource(a.name))} aria-label={`Remove ${a.name}`}>Remove</Button>
          </span>
        </div>
      ))}
      {available.map((d) => (
        <div key={d.name} style={row}>
          <span style={{ fontSize: 12.5 }}>{d.name}</span>
          <Button variant="secondary" onClick={() => run(() => ndiAddSource(d.name))} aria-label={`Add ${d.name}`}>Add</Button>
        </div>
      ))}
      {!unavailable && added.length === 0 && available.length === 0 && (
        <p style={{ ...note, color: 'var(--color-text-muted)' }}>No NDI sources found on the network</p>
      )}
    </div>
  );
}
