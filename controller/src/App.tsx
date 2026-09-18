// controller/src/App.tsx
import { useEffect, useState } from 'react';
import { getBuildInfo } from './lib/ipc';
import { PresentButton } from './components/PresentButton';
import { SlideDeckPanel } from './components/SlideDeckPanel';
import { SceneMediaPanel } from './components/SceneMediaPanel';

interface BuildInfo {
  build_timestamp: string;
  version: string;
}

export default function App() {
  const [buildInfo, setBuildInfo] = useState<BuildInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    getBuildInfo()
      .then(setBuildInfo)
      .catch((e: unknown) => setError(String(e)));
  }, []);

  return (
    <div>
      <PresentButton />
      <SlideDeckPanel />
      <SlideDeckPanel />
      <SceneMediaPanel />

      <h2>Build Info</h2>
      {error && <p>IPC error: {error}</p>}
      {buildInfo ? (
        <ul>
          <li><strong>Timestamp</strong> {buildInfo.build_timestamp}</li>
          <li><strong>Version</strong> v{buildInfo.version}</li>
        </ul>
      ) : (
        <p>Loading…</p>
      )}
      <p>Window: <strong>controller</strong> · 1280 × 800</p>
    </div>
  );
}