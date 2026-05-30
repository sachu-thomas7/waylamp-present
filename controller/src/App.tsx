import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface BuildInfo {
  build_timestamp: string;
  version: string;
}

function App() {
  const [buildInfo, setBuildInfo] = useState<BuildInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<BuildInfo>("get_build_info")
      .then(setBuildInfo)
      .catch((e: unknown) => setError(String(e)));
  }, []);

  return (
    <main style={{ padding: "var(--space-6)" }}>
      <header style={{ marginBottom: "var(--space-4)" }}>
        <h1 style={{ fontSize: "1.5rem", fontWeight: 700, color: "var(--color-primary)" }}>
          StreamDeck — Controller
        </h1>
        <p style={{ color: "var(--color-text-muted)", fontSize: "0.875rem", marginTop: "var(--space-2)" }}>
          Phase 0 · F0.1 Bootstrap
        </p>
      </header>

      <section
        style={{
          background: "var(--color-surface)",
          border: "1px solid var(--color-border)",
          borderRadius: "var(--radius-md)",
          padding: "var(--space-4)",
        }}
      >
        <h2 style={{ fontSize: "1rem", marginBottom: "var(--space-2)" }}>Build Info</h2>
        {error && <p style={{ color: "#f87171" }}>IPC error: {error}</p>}
        {buildInfo ? (
          <dl style={{ display: "grid", gridTemplateColumns: "auto 1fr", gap: "var(--space-2) var(--space-4)" }}>
            <dt style={{ color: "var(--color-text-muted)" }}>Timestamp</dt>
            <dd>{buildInfo.build_timestamp}</dd>
            <dt style={{ color: "var(--color-text-muted)" }}>Version</dt>
            <dd>v{buildInfo.version}</dd>
          </dl>
        ) : (
          !error && <p style={{ color: "var(--color-text-muted)" }}>Loading…</p>
        )}
      </section>

      <p style={{ marginTop: "var(--space-4)", fontSize: "0.75rem", color: "var(--color-text-muted)" }}>
        Window: <strong>controller</strong> · 1280 × 800
      </p>
    </main>
  );
}

export default App;
