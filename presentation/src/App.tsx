import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface BuildInfo {
  build_timestamp: string;
  version: string;
}

/**
 * F0.1 placeholder slide — satisfies the acceptance criterion
 * "both windows render their respective React apps and show the build timestamp".
 *
 * This component will be replaced by the full SlideCanvas in Phase 1 (F1.1).
 */
function App() {
  const [buildInfo, setBuildInfo] = useState<BuildInfo | null>(null);

  useEffect(() => {
    invoke<BuildInfo>("get_build_info")
      .then(setBuildInfo)
      .catch(console.error);
  }, []);

  return (
    <div
      style={{
        width: "1920px",
        height: "1080px",
        /* Scale the 1920×1080 canvas to fill whatever the window size is */
        transform: `scale(${Math.min(
          window.innerWidth / 1920,
          window.innerHeight / 1080
        )})`,
        transformOrigin: "top left",
        background: "#111",
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        gap: "1rem",
      }}
    >
      <h1 style={{ fontSize: "4rem", fontWeight: 700, letterSpacing: "-0.02em", color: "#4f98a3" }}>
        StreamDeck
      </h1>
      <p style={{ fontSize: "1.5rem", color: "#8b8b96" }}>Presentation Window · F0.1 Bootstrap</p>
      {buildInfo && (
        <p style={{ fontSize: "1rem", color: "#555", marginTop: "2rem" }}>
          Build: {buildInfo.build_timestamp} · v{buildInfo.version}
        </p>
      )}
      <p style={{ fontSize: "0.875rem", color: "#333", marginTop: "1rem" }}>
        1920 × 1080 · Phase 1 will render slide content here
      </p>
    </div>
  );
}

export default App;
