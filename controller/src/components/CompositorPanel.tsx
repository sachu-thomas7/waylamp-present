import { useEffect, useState } from 'react';
import { IDLE_COMPOSITOR, useCompositorStore } from '../store/compositorStore';
import { onAppEvent } from '../lib/events';
import {
  compositorGetState, compositorListScenes, compositorSetLayout, compositorStart, compositorStop,
  type SourceGeometry,
} from '../lib/ipc';
import { Badge } from './ui/Badge';
import { Button } from './ui/Button';

const heading = { fontSize: 11.5, fontWeight: 600, letterSpacing: '0.07em', textTransform: 'uppercase', color: 'var(--color-text-muted)' } as const;
const row = { display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 8 } as const;
const note = { fontSize: 11.5, margin: 0 } as const;

export const DEMO_SCENE = '';
const MAX_SOURCES = 8;

// Next free "cam-N" as a 640x360 thumbnail in the top-right corner, above everything else.
export function nextTestSource(sources: SourceGeometry[]): SourceGeometry {
  let n = sources.length + 1;
  while (sources.some((s) => s.name === `cam-${n}`)) n += 1;
  const z = sources.reduce((max, s) => Math.max(max, s.z), 0) + 1;
  return { name: `cam-${n}`, x: 1280, y: 0, w: 640, h: 360, z, kind: 'test' };
}

export function CompositorPanel() {
  const { status, scenes, lastError, setStatus, setScenes, setLastError } = useCompositorStore();
  const [scene, setScene] = useState(DEMO_SCENE);

  useEffect(() => {
    compositorGetState().then(setStatus).catch(console.error);
    compositorListScenes().then(setScenes).catch(console.error);

    const unlisten = onAppEvent((event) => {
      const cur = useCompositorStore.getState().status;
      if (event.type === 'CompositorStateChanged') {
        const { running, element, hardware } = event.payload;
        setStatus(running ? { ...cur, running, element, hardware } : IDLE_COMPOSITOR);
      } else if (event.type === 'CompositorLayoutChanged') {
        setStatus({ ...cur, sources: event.payload.sources });
      } else if (event.type === 'PipelineError') {
        setLastError(`${event.payload.element}: ${event.payload.message}`);
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

  const { sources } = status;

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
      <div style={row}>
        <span style={heading}>Compositor</span>
        <Badge>{status.running ? `${status.hardware ? 'GPU' : 'CPU'} · ${status.element}` : 'Stopped'}</Badge>
      </div>
      {lastError && <p style={{ ...note, color: 'var(--color-warn)' }}>{lastError}</p>}
      {!status.running && (
        <select
          aria-label="Scene"
          value={scene}
          onChange={(e) => setScene(e.target.value)}
          style={{ padding: '6px 8px', borderRadius: 'var(--radius-sm)', border: '1px solid var(--color-border)', background: 'var(--color-surface)', color: 'var(--color-text)', fontSize: 12.5 }}
        >
          <option value={DEMO_SCENE}>Demo (3 test sources)</option>
          {scenes.map((s) => <option key={s} value={s}>{s}</option>)}
        </select>
      )}
      {status.running && sources.map((s) => (
        <div key={s.name} style={row} data-testid={`compositor-source-${s.name}`}>
          <span style={{ fontSize: 12.5 }}>{s.name}</span>
          <span style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
            <span style={{ fontSize: 11.5, fontFamily: 'var(--font-mono)', color: 'var(--color-text-muted)' }}>{s.w}x{s.h} @ {s.x},{s.y}</span>
            <Button
              variant="secondary"
              aria-label={`Remove ${s.name}`}
              onClick={() => run(() => compositorSetLayout(sources.filter((o) => o.name !== s.name)))}
            >
              Remove
            </Button>
          </span>
        </div>
      ))}
      <div style={{ display: 'flex', gap: 6 }}>
        {!status.running && (
          <Button variant="secondary" onClick={() => run(() => compositorStart(scene === DEMO_SCENE ? null : scene))}>Start</Button>
        )}
        {status.running && (
          <Button
            variant="secondary"
            disabled={sources.length >= MAX_SOURCES}
            onClick={() => run(() => compositorSetLayout([...sources, nextTestSource(sources)]))}
          >
            Add test source
          </Button>
        )}
        {status.running && <Button variant="secondary" onClick={() => run(compositorStop)}>Stop</Button>}
      </div>
    </div>
  );
}
