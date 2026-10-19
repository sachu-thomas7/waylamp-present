import { useEffect } from 'react';
import { usePipelineStore } from '../store/pipelineStore';
import { onAppEvent } from '../lib/events';
import { pipelineGetState, pipelinePause, pipelineResume, pipelineStartTest, pipelineStop } from '../lib/ipc';
import { Badge } from './ui/Badge';
import { Button } from './ui/Button';

export function PipelinePanel() {
  const { state, lastError, setState, setLastError } = usePipelineStore();

  useEffect(() => {
    pipelineGetState().then(setState).catch(console.error);

    const unlisten = onAppEvent((event) => {
      if (event.type === 'PipelineStateChanged') {
        setState(event.payload.state);
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

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
        <span style={{ fontSize: 11.5, fontWeight: 600, letterSpacing: '0.07em', textTransform: 'uppercase', color: 'var(--color-text-muted)' }}>
          Test Pipeline
        </span>
        <Badge>{state === 'Null' ? 'Stopped' : state}</Badge>
      </div>
      {lastError && (
        <p style={{ fontSize: 11.5, color: 'var(--color-warn)', margin: 0 }}>{lastError}</p>
      )}
      <div style={{ display: 'flex', gap: 6 }}>
        {state === 'Null' && <Button variant="secondary" onClick={() => run(pipelineStartTest)}>Start</Button>}
        {state === 'Playing' && <Button variant="secondary" onClick={() => run(pipelinePause)}>Pause</Button>}
        {state === 'Paused' && <Button variant="secondary" onClick={() => run(pipelineResume)}>Resume</Button>}
        {state !== 'Null' && <Button variant="secondary" onClick={() => run(pipelineStop)}>Stop</Button>}
      </div>
    </div>
  );
}
