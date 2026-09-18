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
    onAppEvent((event) => {
      if (event.type === 'SceneSourceChanged') {
        setSceneSource(event.payload.source);
      } else if (event.type === 'SceneMediaUpdate') {
        setSceneMedia(event.payload);
      }
    });
    const unlistenPromise = onAppEvent((event) => {
      if (event.type === 'SlideUpdate') {
        setSlide(event.payload);
      }
    });
    return () => {
      unlistenPromise.then((fn) => fn());
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