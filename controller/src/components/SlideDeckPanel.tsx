import { memo, useCallback, useDeferredValue, useEffect, useMemo, useRef, useState } from 'react';
import { useControllerSlideStore } from '../store/slideStore';
import { onAppEvent } from '../lib/events';
import {
  slideAdvance, slideRewind, slideJumpTo, slideGetCurrent, slideTimerReset, pickDeckFile, deckImportStage, deckImportCommit, deckImportDiscard, deckGetAllSlides,
} from '../lib/ipc';
import type { ImportPreview, SlidePayload } from '../lib/ipc';
import { SlideCanvas } from './SlideCanvas';
import { Card } from './ui/Card';
import { Button } from './ui/Button';
import { IconButton } from './ui/IconButton';
import { Badge } from './ui/Badge';
import { IconChevronLeft, IconChevronRight, IconReset, IconSearch, IconUpload, IconWarning } from './ui/icons';

const THUMB_SCALE = 160 / 1920;

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

function errorMessage(e: unknown): string {
  if (typeof e === 'object' && e !== null && 'message' in e) return String(e.message);
  return String(e);
}

// slide 0 means the whole file failed, not one slide.
interface SlideThumbProps {
  index: number;
  slide: SlidePayload | undefined;
  active: boolean;
  onSelect: (index: number) => void;
}

const SlideThumb = memo(function SlideThumb({ index, slide, active, onSelect }: SlideThumbProps) {
  return (
  <button
              type="button"
        aria-label={`Slide ${index + 1}`}
        aria-current={active ? 'true' : undefined}
        onClick={(e) => {
          onSelect(index);
          e.currentTarget.blur();
        }}
        onKeyDown={(e) => {
          if (e.key === ' ') e.stopPropagation();
        }}
        style={{
          flexShrink: 0,
          padding: 0,
          border: 'none',
          background: 'none',
          font: 'inherit',
          borderRadius: 8,
          cursor: 'pointer',
          overflow: 'hidden',
          position: 'relative',
          outline: active ? '2px solid var(--color-primary)' : '1px solid var(--color-border)',
          boxShadow: active ? '0 0 0 3px rgba(79,152,163,0.22)' : undefined,
        }}
      >
        {slide && (
          <>
            <SlideCanvas slide={slide} scale={THUMB_SCALE} />
            {slide.layout_unsupported && (
              <div
                role="img"
                aria-label="Unsupported layout — imported with blank body"
                title="Unsupported layout — imported with blank body"
                style={{ position: 'absolute', top: 5, right: 5, color: 'var(--color-warn)' }}
              >
                <IconWarning />
              </div>
            )}
          </>
        )}
      </button>
  );
});

function importErrorText({ slide, reason }: { slide: number; reason: string }): string {
  return slide > 0 ? `Slide ${slide}: ${reason}` : reason;
}

export function SlideDeckPanel() {
  const {
    current, slides, atDeckStart, atDeckEnd, elapsedSecs,
    setSlide, setSlides, setAtDeckEnd, setAtDeckStart, setElapsedSecs, resetTimer,
    importProgress, importError, setImportProgress, setImportError,
  } = useControllerSlideStore();

  const [jumpIndex, setJumpIndex] = useState('');
  const [filter, setFilter] = useState('');
  const deferredFilter = useDeferredValue(filter);
  const [pending, setPending] = useState<ImportPreview | null>(null);

  useEffect(() => {
    slideGetCurrent()
      .then((slide) => { if (slide) setSlide(slide); })
      .catch(console.error);
    deckGetAllSlides().then(setSlides).catch(console.error);
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
      const t = e.target;
      if (t instanceof HTMLElement && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.isContentEditable)) return;
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
  const stripRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = stripRef.current?.querySelector('[aria-current="true"]');
    el?.scrollIntoView?.({ block: 'nearest', inline: 'nearest' });
  }, [currentIndex, slides.length]);

  const jumpNumber = /^[0-9]+$/.test(jumpIndex) ? Number(jumpIndex) : NaN;
  const jumpTarget = jumpNumber >= 1 && jumpNumber <= totalSlides ? jumpNumber - 1 : null;

  useEffect(() => {
    const unlisten = onAppEvent((event) => {
      if (event.type === 'DeckImportProgress') {
        setImportProgress(event.payload);
      } else if (event.type === 'DeckImportComplete') {
        setImportProgress(null);
        deckGetAllSlides().then(setSlides).catch(console.error);
      } else if (event.type === 'DeckImportFailed') {
        setImportError(event.payload);
      }
    });
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  async function handleImport() {
    const path = await pickDeckFile();
    if (path !== null) {
      setImportError(null);
      if (pending) deckImportDiscard(pending.staging_id).catch(console.error);
      setPending(null);
      await deckImportStage(path)
        .then(setPending)
        .catch((e) => {
          console.error(e);
          setImportError({ slide: 0, reason: errorMessage(e) });
        });
      setImportProgress(null);
    }
  }

  async function confirmImport() {
    if (!pending) return;
    const id = pending.staging_id;
    setPending(null);
    await deckImportCommit(id).catch((e) => {
      console.error(e);
      setImportError({ slide: 0, reason: errorMessage(e) });
    });
  }

  function cancelImport() {
    if (!pending) return;
    deckImportDiscard(pending.staging_id).catch(console.error);
    setPending(null);
  }

  const pendingCard = pending && (
    <div
      role="group"
      aria-label="Pending import"
      style={{ display: 'flex', flexDirection: 'column', gap: 8, padding: 12, border: '1px solid var(--color-primary)', borderRadius: 'var(--radius-sm)', background: 'var(--color-bg)', textAlign: 'left' }}
    >
      <span style={{ fontSize: 13, fontWeight: 600 }}>
        Ready to import {pending.slide_count} {pending.slide_count === 1 ? 'slide' : 'slides'}
        {pending.skipped > 0 && ' (' + pending.skipped + ' could not be read and will be left out)'}
      </span>
      <span style={{ fontSize: 11.5, color: 'var(--color-text-muted)' }}>
        {pending.first_titles.filter(Boolean).join(' · ')}
      </span>
      <span style={{ fontSize: 11.5, color: 'var(--color-text-muted)' }}>
        Your current deck stays live until you choose to use this one.
      </span>
      <div style={{ display: 'flex', gap: 8 }}>
        <Button variant="primary" onClick={confirmImport}>Use this deck</Button>
        <Button variant="secondary" onClick={cancelImport}>Cancel</Button>
      </div>
    </div>
  );

  const jumpTo = useCallback((i: number) => {
    slideJumpTo(i).catch(console.error);
  }, []);

  const filteredIndices = useMemo(() => {
    if (!deferredFilter.trim()) return slides.map((_, i) => i);
    const q = deferredFilter.trim().toLowerCase();
    return slides.reduce<number[]>((acc, s, i) => {
      if (s.title.toLowerCase().includes(q)) acc.push(i);
      return acc;
    }, []);
  }, [slides, deferredFilter]);

  if (totalSlides === 0) {
    return (
      <Card title="Slide Deck" headerRight={<Badge>0 slides</Badge>}>
        <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 16, padding: '40px 20px', textAlign: 'center' }}>
          <div style={{ width: 56, height: 56, borderRadius: 14, background: 'var(--color-bg)', border: '1px solid var(--color-border)', display: 'flex', alignItems: 'center', justifyContent: 'center', color: 'var(--color-text-muted)' }}>
            <svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round">
              <rect x="3" y="3" width="13" height="9" rx="1.4" />
              <rect x="7" y="8" width="14" height="10" rx="1.4" fill="var(--color-surface)" />
              <path d="M14 11v6M11 14h6" />
            </svg>
          </div>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 6, maxWidth: 340 }}>
            <span style={{ fontSize: 16, fontWeight: 700 }}>No slides yet</span>
            <span style={{ fontSize: 12.5, color: 'var(--color-text-muted)', lineHeight: 1.6 }}>
              Import a PowerPoint or PDF file to build your deck — StreamDeck extracts titles, body text, and backgrounds automatically.
            </span>
          </div>
          <Button variant="primary" pill icon={<IconUpload />} onClick={handleImport}>Import a Deck</Button>
          {pendingCard}
          {importError && (
            <span style={{ fontSize: 11.5, color: 'var(--color-warn)' }}>⚠ {importErrorText(importError)}</span>
          )}
          {importProgress && (
            <span style={{ fontSize: 11.5, color: 'var(--color-text-muted)' }}>Importing… slide {importProgress.slide} / {importProgress.total}</span>
          )}
        </div>
      </Card>
    );
  }

  return (
    <div className="deck-panel">
    <Card
      fill
      title="Slide Deck"
      headerRight={<Badge>{totalSlides ? `${currentIndex + 1} / ${totalSlides}` : '-- / --'}</Badge>}
    >
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 12, marginTop: -6 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 6, boxSizing: 'border-box', height: 32, padding: '0 10px', background: 'var(--color-bg)', border: '1px solid var(--color-border)', borderRadius: 'var(--radius-sm)', width: 150 }}>
          <IconSearch style={{ color: 'var(--color-text-muted)', flexShrink: 0 }} />
          <input
            aria-label="Filter slides"
            placeholder="Filter slides…"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            style={{ border: 'none', background: 'transparent', color: 'var(--color-text)', fontSize: 12, outline: 'none', width: '100%' }}
          />
        </div>
        <Button variant="secondary" icon={<IconUpload />} onClick={handleImport}>Import Deck</Button>
      </div>
      {pendingCard}
      {importProgress && (
        <span style={{ fontSize: 11.5, color: 'var(--color-text-muted)' }}>Importing… slide {importProgress.slide} / {importProgress.total}</span>
      )}
      {importError && (
        <span style={{ fontSize: 11.5, color: 'var(--color-warn)' }}>⚠ {importErrorText(importError)}</span>
      )}

      {/* Thumbnail strip */}
      <div ref={stripRef} style={{ display: 'flex', gap: 10, overflowX: 'auto', paddingBottom: 2 }}>
        {filteredIndices.map((i) => (
          <SlideThumb key={i} index={i} slide={slides[i]} active={i === currentIndex} onSelect={jumpTo} />
        ))}
      </div>

      {/* Current + Next preview */}
      <div className="preview-grid">
        <div data-testid="preview-current" style={{ minWidth: 0 }}>
          <div style={{ fontSize: 11, fontWeight: 700, letterSpacing: '0.08em', textTransform: 'uppercase', color: 'var(--color-text-muted)', marginBottom: 6 }}>Current</div>
          {current && <SlideCanvas slide={current} fluid />}
        </div>
        <div data-testid="preview-next" style={{ minWidth: 0 }}>
          <div style={{ fontSize: 11, fontWeight: 700, letterSpacing: '0.08em', textTransform: 'uppercase', color: 'var(--color-text-muted)', marginBottom: 6 }}>Next</div>
          {slides[currentIndex + 1] && (
            <SlideCanvas slide={slides[currentIndex + 1]} fluid />
          )}
        </div>
      </div>

      <div style={{ fontSize: 11.5, color: 'var(--color-text-muted)' }}>
        {current?.notes ? `Notes — ${current.notes}` : 'No notes for this slide'}
      </div>

      {/* Timer + nav */}
      <div className="nav-bar" style={{ paddingTop: 12, borderTop: '1px solid var(--color-border)', marginTop: 'auto' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 10, flexShrink: 0 }}>
          <span style={{ fontFamily: 'var(--font-mono)', fontSize: 19, fontWeight: 600, letterSpacing: '0.02em', minWidth: 64 }}>
            {formatTimer(elapsedSecs)}
          </span>
          <IconButton title="Reset timer" onClick={() => slideTimerReset().catch(console.error)}>
            <IconReset />
          </IconButton>
        </div>

        {/* Boundary banners */}
        {atDeckStart && <span style={{ color: 'var(--color-warn)', fontSize: 12, flexShrink: 0 }}>◀ Start of deck</span>}
        {atDeckEnd && <span style={{ color: 'var(--color-warn)', fontSize: 12, flexShrink: 0 }}>End of deck ▶</span>}

        {/* Primary: Prev/Next, adjacent and prominent — this is what an operator clicks constantly */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, flexShrink: 0 }}>
          <Button variant="secondary" icon={<IconChevronLeft />} onClick={() => slideRewind().catch(console.error)}>
            Prev
          </Button>
          <Button variant="secondary" icon={<IconChevronRight />} onClick={() => slideAdvance().catch(console.error)} style={{ flexDirection: 'row-reverse' }}>
            Next
          </Button>
        </div>

        {/* Secondary: jump to an arbitrary slide by number (1-based, matching the "n / total" badge) — smaller than Prev/Next but still clearly legible */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 6, flexShrink: 0 }}>
          <span style={{ fontSize: 11.5, color: 'var(--color-text-muted)' }}>Jump to</span>
          <input
            type="number"
            aria-label="Jump to slide number"
            value={jumpIndex}
            onChange={(e) => setJumpIndex(e.target.value)}
            placeholder="#"
            min={1}
            max={totalSlides}
            style={{ boxSizing: 'border-box', width: 56, height: 32, padding: 0, lineHeight: '30px', textAlign: 'center', background: 'var(--color-bg)', border: '1px solid var(--color-border)', borderRadius: 'var(--radius-sm)', color: 'var(--color-text)', fontSize: 12.5 }}
          />
          <Button
            variant="secondary"
            disabled={jumpTarget === null}
            onClick={() => {
              if (jumpTarget !== null) slideJumpTo(jumpTarget).catch(console.error);
            }}
          >
            Go
          </Button>
        </div>
      </div>
    </Card>
    </div>
  );
}
