// presentation/src/App.tsx
import { useState, useEffect } from 'react';
import { onAppEvent } from './lib/events';
import { useSlideStore } from './store/slideStore';
import { SlideCanvas } from './components/SlideCanvas';
import { SceneMediaView } from './components/SceneMediaView';
import type { SceneMediaPayload } from './lib/ipc';

export default function App() {
  const setSlide = useSlideStore((s) => s.setSlide);
  const [sceneSource, setSceneSource] = useState<'SlideDeck' | 'SceneMedia'>('SlideDeck');
  const [sceneMedia, setSceneMedia] = useState<SceneMediaPayload | null>(null);

  useEffect(() => {
    let dead = false;
    let off: (() => void) | undefined;
    // BusLag needs no pull here: the projector cannot invoke, so Rust re-sends a full snapshot right after it.
    onAppEvent((event) => {
      if (event.type === 'SceneSourceChanged') {
        setSceneSource(event.payload.source);
      } else if (event.type === 'SceneMediaUpdate') {
        setSceneMedia(event.payload);
      } else if (event.type === 'SlideUpdate') {
        setSlide(event.payload);
      }
    }).then((fn) => {
      if (dead) fn();
      else off = fn;
    });
    return () => {
      dead = true;
      off?.();
    };
  }, [setSlide]);

  return (
    sceneSource === 'SceneMedia' && sceneMedia ? (
      <SceneMediaView media={sceneMedia} />
    ) : (
      <SlideCanvas />
    )
  );
}