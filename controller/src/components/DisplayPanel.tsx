import { useEffect, useState } from 'react';
import { useDisplayStore } from '../store/displayStore';
import { onAppEvent } from '../lib/events';
import { displayList, displayAssignPresentation } from '../lib/ipc';

export function DisplayPanel() {
  const [isCollapsed, setIsCollapsed] = useState(false);
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
    <section>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <span>▼ Displays</span>
        <button onClick={() => setIsCollapsed((c) => !c)}>
          {isCollapsed ? '▼ Expand' : '▲ Collapse'}
        </button>
      </div>

      {!isCollapsed && (
        <>
          {lostWarning && (
            <p>⚠ Assigned display disconnected — presentation fell back to the primary display.</p>
          )}
          <ul>
            {displays.map((d) => (
              <li key={d.id}>
                {d.name} — {d.width}×{d.height} at ({d.x}, {d.y}){d.is_primary ? ' (primary)' : ''}
                {' '}
                <button disabled={assignedDisplayId === d.id} onClick={() => handleAssign(d.id)}>
                  {assignedDisplayId === d.id ? 'Assigned' : 'Assign to Presentation'}
                </button>
              </li>
            ))}
            {displays.length === 0 && <li>No displays detected</li>}
          </ul>
        </>
      )}
    </section>
  );
}
