import { useState, useEffect } from 'react';
import { presentationShow, presentationHide } from '../lib/ipc';
import { onAppEvent } from '../lib/events';
import { Button } from './ui/Button';
import { IconPlay, IconStop } from './ui/icons';
import { useControllerSlideStore } from '../store/slideStore';

export function PresentButton() {
  const [presenting, setPresenting] = useState(false);
  const totalSlides = useControllerSlideStore((s) => s.current?.total ?? 0);
  const nothingToPresent = totalSlides === 0 && !presenting;

  useEffect(() => {
    const unlistenPromise = onAppEvent((event) => {
      if (event.type === 'PresentationVisibility') {
        setPresenting(event.payload.visible);
      }
    });
    return () => {
      unlistenPromise.then((fn) => fn());
    };
  }, []);

  async function handleClick() {
    try {
      // Optimistic UI update — Rust will confirm via PresentationVisibility event
      if (presenting) {
        await presentationHide();
      } else {
        await presentationShow();
      }
    } catch (e: unknown) {
      console.error('Presentation toggle failed:', e);
    }
  }

  return (
    <Button
      type="button"
      pill
      variant={presenting ? 'live' : 'primary'}
      icon={presenting ? <IconStop /> : <IconPlay />}
      onClick={handleClick}
      disabled={nothingToPresent}
      title={nothingToPresent ? 'Import a deck to enable presenting' : undefined}
    >
      {presenting ? 'Stop Presenting' : 'Present'}
    </Button>
  );
}
