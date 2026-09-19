import { useState, useEffect } from 'react';
import { presentationShow, presentationHide } from '../lib/ipc';
import { onAppEvent } from '../lib/events';
import { Button } from './ui/Button';
import { IconPlay, IconStop } from './ui/icons';

export function PresentButton() {
  const [presenting, setPresenting] = useState(false);

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
    >
      {presenting ? 'Stop Presenting' : 'Present'}
    </Button>
  );
}
