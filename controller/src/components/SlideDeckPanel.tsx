import { useEffect, useState } from 'react';
import { useControllerSlideStore } from '../store/slideStore';
import { onAppEvent } from '../lib/events';
import {
  slideAdvance, slideRewind, slideJumpTo, slideGetCurrent, slideTimerReset, pickDeckFile, deckImport, deckGetAllSlides,
} from '../lib/ipc';
import { SlideCanvas } from './SlideCanvas';

const THUMB_SCALE = 160 / 1920;
const CURRENT_PREVIEW_SCALE = 380 / 1920;
const NEXT_PREVIEW_SCALE = 190 / 1920;

function formatTimer(secs: number | null): string {
  if (secs === null) return '--:--';
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  const mm = String(m).padStart(2, '0');
  const ss = String(s).padStart(2, '0');
  if (h > 0) return `${String(h).padStart(2, '0')}:${mm}:${ss}`;
  return `${mm}:${ss}`;
}

export function SlideDeckPanel() {
  const {
    current, atDeckStart, atDeckEnd, elapsedSecs, isCollapsed,
    setSlide, setAtDeckEnd, setAtDeckStart, setElapsedSecs, resetTimer, toggleCollapse,
    importProgress, importError, setImportProgress, setImportError,
  } = useControllerSlideStore();

  const [jumpIndex, setJumpIndex] = useState('');

  useEffect(() => {
    slideGetCurrent()
      .then((slide) => { if (slide) setSlide(slide); })
      .catch(console.error);
  }, []);

  useEffect(() => {
    const unlisten = onAppEvent((event) => {
      if (event.type === 'SlideUpdate') setSlide(event.payload);
      else if (event.type === 'DeckEnd') setAtDeckEnd(true);
      else if (event.type === 'DeckStart') setAtDeckStart(true);
      else if (event.type === 'TimerTick') setElapsedSecs(event.payload.elapsed_secs);
      else if (event.type === 'TimerReset') resetTimer();
    });
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === ' ' || e.key === 'ArrowRight') {
        e.preventDefault();
        slideAdvance().catch(console.error);
      } else if (e.key === 'ArrowLeft') {
        slideRewind().catch(console.error);
      }
    };
    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
  }, []);

  const totalSlides = current?.total ?? 0;
  const currentIndex = current?.index ?? 0;

  useEffect(() => {
    const unlisten = onAppEvent((event) => {
      if (event.type === 'DeckImportProgress') {
        setImportProgress(event.payload);
      } else if (event.type === 'DeckImportComplete') {
        setImportProgress(null);
        deckGetAllSlides().then(/* repopulate thumbnail state */).catch(console.error);
      } else if (event.type === 'DeckImportFailed') {
        setImportError(event.payload);
      }
    });
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  async function handleImport() {
    const path = await pickDeckFile();
    if (path !== null) {
      await deckImport(path).catch(console.error);
    }
  }

  return (
    <section style={{ border: '1px solid #333', borderRadius: 8, margin: 8, overflow: 'hidden' }}>
      <div style={{
        display: 'flex', justifyContent: 'space-between', alignItems: 'center',
        padding: '8px 12px', background: '#222', cursor: 'default',
      }}>
        <span style={{ color: '#ccc', fontWeight: 600 }}>▼ Slide Deck</span>
        <button onClick={toggleCollapse} style={{ fontSize: 12 }}>
          {isCollapsed ? '▼ Expand' : '▲ Collapse'}
        </button>
      </div>

      {!isCollapsed && (
        <>
          <div>
            <button onClick={handleImport}>Import Deck</button>
            {importProgress && (
              <span>Importing… slide {importProgress.slide} / {importProgress.total}</span>
            )}
            {importError && (
              <span>⚠ Slide {importError.slide}: {importError.reason}</span>
            )}
          </div>
          {/* Thumbnail grid */}
          <div style={{ display: 'flex', overflowX: 'auto', gap: 6, padding: 8, background: '#111' }}>
            {Array.from({ length: totalSlides }, (_, i) => (
              <div
                key={i}
                onClick={() => slideJumpTo(i).catch(console.error)}
                style={{
                  outline: i === currentIndex ? '2px solid #4af' : '1px solid #444',
                  cursor: 'pointer', flexShrink: 0, borderRadius: 3, overflow: 'hidden',
                }}
              >
                {current && (
                  <SlideCanvas
                    slide={{ ...current, index: i, title: current.title, body: current.body }}
                    scale={THUMB_SCALE}
                  />
                )}
              </div>
            ))}
          </div>

          {/* Current + Next preview */}
          <div style={{ display: 'flex', gap: 8, padding: 8, background: '#181818' }}>
            <div>
              <div style={{ fontSize: 11, color: '#888', marginBottom: 4 }}>Current</div>
              {current && <SlideCanvas slide={current} scale={CURRENT_PREVIEW_SCALE} />}
            </div>
            <div>
              <div style={{ fontSize: 11, color: '#888', marginBottom: 4 }}>Next</div>
              {current && currentIndex < totalSlides - 1 && (
                <SlideCanvas
                  slide={{ ...current, index: currentIndex + 1 }}
                  scale={NEXT_PREVIEW_SCALE}
                />
              )}
            </div>
          </div>

          {/* Speaker notes */}
          <textarea
            readOnly
            value={current?.notes ?? ''}
            rows={3}
            style={{
              width: '100%', resize: 'none', background: '#111', color: '#ddd',
              padding: '6px 8px', boxSizing: 'border-box', border: 'none',
              borderTop: '1px solid #2a2a2a', fontFamily: 'inherit', fontSize: 13,
            }}
          />

          {/* Timer + counter */}
          <div style={{
            display: 'flex', alignItems: 'center', gap: 10,
            padding: '4px 8px', background: '#1a1a1a', borderTop: '1px solid #2a2a2a',
          }}>
            <span style={{ fontVariantNumeric: 'tabular-nums', color: '#ccc' }}>
              ⏱ {formatTimer(elapsedSecs)}
            </span>
            <button onClick={() => slideTimerReset().catch(console.error)}>Reset</button>
            <span style={{ marginLeft: 'auto', color: '#888' }}>
              {current ? `${currentIndex + 1} / ${totalSlides}` : '-- / --'}
            </span>
          </div>

          {/* Boundary banners */}
          {atDeckStart && (
            <div style={{ color: '#fa0', padding: '2px 8px', fontSize: 12 }}>◀ Start of deck</div>
          )}
          {atDeckEnd && (
            <div style={{ color: '#fa0', padding: '2px 8px', fontSize: 12 }}>End of deck ▶</div>
          )}

          {/* Navigation */}
          <div style={{ display: 'flex', gap: 6, padding: 8, background: '#1a1a1a' }}>
            <button onClick={() => slideRewind().catch(console.error)}>← Prev</button>
            <button onClick={() => slideAdvance().catch(console.error)}>Next →</button>
            <input
              type="number"
              value={jumpIndex}
              onChange={(e) => setJumpIndex(e.target.value)}
              style={{ width: 48 }}
              placeholder="idx"
              min={0}
              max={totalSlides - 1}
            />
            <button
              onClick={() => {
                const idx = parseInt(jumpIndex, 10);
                if (!Number.isNaN(idx)) slideJumpTo(idx).catch(console.error);
              }}
            >
              Jump
            </button>
          </div>
        </>
      )}
    </section>
  );
}