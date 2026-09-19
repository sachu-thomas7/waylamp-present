import { useEffect } from 'react';
import { useDisplayStore } from '../store/displayStore';
import { onAppEvent } from '../lib/events';
import { displayList, displayAssignPresentation } from '../lib/ipc';
import { Button } from './ui/Button';
import { IconCheck, IconMonitor } from './ui/icons';

export function DisplayPanel() {
  const {
    displays, assignedDisplayId, lostWarning,
    setDisplays, setAssignedDisplayId, setLostWarning,
  } = useDisplayStore();

  useEffect(() => {
    displayList().then(setDisplays).catch(console.error);

    const unlisten = onAppEvent((event) => {
      if (event.type === 'DisplaysChanged') {
        setDisplays(event.payload);
      } else if (event.type === 'DisplayAssigned') {
        setAssignedDisplayId(event.payload.display_id);
        setLostWarning(false);
      } else if (event.type === 'DisplayLost') {
        setLostWarning(true);
      }
    });

    return () => { unlisten.then((fn) => fn()); };
  }, []);

  async function handleAssign(id: string) {
    await displayAssignPresentation(id).catch(console.error);
    setAssignedDisplayId(id);
    setLostWarning(false);
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
      <span style={{ fontSize: 11.5, fontWeight: 600, letterSpacing: '0.07em', textTransform: 'uppercase', color: 'var(--color-text-muted)' }}>
        Displays
      </span>
      {lostWarning && (
        <p style={{ fontSize: 11.5, color: 'var(--color-warn)', margin: 0 }}>
          ⚠ Assigned display disconnected — presentation fell back to the primary display.
        </p>
      )}
      {displays.length === 0 && (
        <span style={{ fontSize: 12, color: 'var(--color-text-muted)' }}>No displays detected</span>
      )}
      {displays.map((d) => {
        const assigned = assignedDisplayId === d.id;
        return (
          <div key={d.id} style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 10 }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 10, minWidth: 0 }}>
              <div style={{ width: 32, height: 32, borderRadius: 8, background: 'var(--color-bg)', border: '1px solid var(--color-border)', display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0, color: 'var(--color-text-muted)' }}>
                <IconMonitor />
              </div>
              <div style={{ display: 'flex', flexDirection: 'column', lineHeight: 1.3, minWidth: 0 }}>
                <span style={{ fontSize: 12.5, fontWeight: 600 }}>{d.name}</span>
                <span style={{ fontSize: 11, color: 'var(--color-text-muted)' }}>
                  {d.width}×{d.height}{d.is_primary ? ' · Primary' : ''}
                </span>
              </div>
            </div>
            {assigned ? (
              <div style={{ flexShrink: 0, display: 'flex', alignItems: 'center', gap: 5, padding: '5px 10px', background: 'rgba(79,152,163,0.14)', borderRadius: 7, color: 'var(--color-primary)' }}>
                <IconCheck />
                <span style={{ fontSize: 11, fontWeight: 600 }}>Assigned</span>
              </div>
            ) : (
              <Button variant="secondary" onClick={() => handleAssign(d.id)}>Assign</Button>
            )}
          </div>
        );
      })}
    </div>
  );
}
