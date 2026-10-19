import { useEffect, useState } from 'react';
import { useNdiStore } from '../store/ndiStore';
import { usePreviewStore } from '../store/previewStore';
import { onAppEvent, onPreviewFrame } from '../lib/events';
import { ndiListSources, previewGetLayout, previewGetSnapshot, previewSetLayout, type PreviewLayout } from '../lib/ipc';
import { refreshWhip } from '../store/whipStore';
import { CameraPreview } from './CameraPreview';
import { Card } from './ui/Card';
import { SegmentedControl } from './ui/SegmentedControl';

// Layout is rows x columns: always two rows, N columns.
const LAYOUTS: Array<{ value: PreviewLayout; label: string; columns: number }> = [
  { value: '2x2', label: '2×2', columns: 2 },
  { value: '2x3', label: '2×3', columns: 3 },
  { value: '2x4', label: '2×4', columns: 4 },
];

const note = { fontSize: 12, margin: 0, color: 'var(--color-text-muted)' } as const;

export function CameraPreviewGrid() {
  const added = useNdiStore((s) => s.added);
  const layout = usePreviewStore((s) => s.layout);
  // Bumping the epoch re-subscribes and re-fetches, which is how the grid recovers after sleep/wake.
  const [epoch, setEpoch] = useState(0);

  useEffect(() => {
    const ndi = useNdiStore.getState();
    const preview = usePreviewStore.getState();
    ndiListSources().then(ndi.setSnapshot).catch(console.error);
    previewGetSnapshot().then(preview.setSnapshot).catch(console.error);
    refreshWhip().catch(console.error);

    const unlistenEvents = onAppEvent((event) => {
      switch (event.type) {
        case 'SourceStats':
          preview.setStats({ ...event.payload });
          break;
        case 'SourceListChanged':
          ndi.setDiscovered(event.payload.sources);
          break;
        case 'SourceLost':
          ndi.setLost(event.payload.name, true);
          break;
        case 'SourceDiscovered':
          ndi.setLost(event.payload.source.name, false);
          break;
        case 'SourceAdded':
          ndi.markAdded(event.payload.name, event.payload.format);
          break;
        case 'SourceRemoved':
          ndi.markRemoved(event.payload.name);
          preview.forget(event.payload.name);
          refreshWhip().catch(console.error);
          break;
        case 'WhipSessionChanged':
          refreshWhip().catch(console.error);
          break;
      }
    });
    const unlistenFrames = onPreviewFrame(preview.setFrame);

    return () => {
      unlistenEvents.then((fn) => fn());
      unlistenFrames.then((fn) => fn());
    };
  }, [epoch]);

  useEffect(() => {
    previewGetLayout().then(usePreviewStore.getState().setLayout).catch(console.error);
    const onVisible = () => {
      if (document.visibilityState === 'visible') setEpoch((e) => e + 1);
    };
    document.addEventListener('visibilitychange', onVisible);
    return () => document.removeEventListener('visibilitychange', onVisible);
  }, []);

  function chooseLayout(next: PreviewLayout) {
    const previous = usePreviewStore.getState().layout;
    usePreviewStore.getState().setLayout(next);
    previewSetLayout(next).catch((e) => {
      console.error(e);
      usePreviewStore.getState().setLayout(previous);
    });
  }

  const current = LAYOUTS.find((l) => l.value === layout) ?? LAYOUTS[0];
  const capacity = current.columns * 2;
  const shown = added.slice(0, capacity);

  return (
    <Card
      title="Cameras"
      headerRight={
        <SegmentedControl options={LAYOUTS.map(({ value, label }) => ({ value, label }))} value={layout} onChange={chooseLayout} />
      }
    >
      {added.length === 0 ? (
        <p style={note}>No cameras added — add NDI sources or enable phone cameras in Settings</p>
      ) : (
        <div data-testid="camera-grid" style={{ display: 'grid', gridTemplateColumns: `repeat(${current.columns}, 1fr)`, gap: 10 }}>
          {shown.map((a) => (
            <CameraPreview key={a.name} name={a.name} lost={a.lost} />
          ))}
        </div>
      )}
      {added.length > capacity && <p style={note}>{added.length - capacity} more source(s) not shown — pick a larger grid</p>}
    </Card>
  );
}
