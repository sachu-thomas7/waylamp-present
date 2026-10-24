import {
  deckGetAllSlides, displayList, sceneGetActiveSource, slideGetCurrent,
} from './ipc';
import { useDisplayStore } from '../store/displayStore';
import { useSceneStore } from '../store/sceneStore';
import { useControllerSlideStore } from '../store/slideStore';

// A BusLag means events were dropped, so pull every pull-able piece of state again. Slide, scene and
// presentation state are also re-pushed by the backend right after the lag event.
export async function resyncFromBackend(): Promise<void> {
  const slides = useControllerSlideStore.getState();
  const results = await Promise.allSettled([
    slideGetCurrent().then((s) => { if (s) slides.setSlide(s); }),
    deckGetAllSlides().then(slides.setSlides),
    sceneGetActiveSource().then(useSceneStore.getState().setActiveSource),
    displayList().then(useDisplayStore.getState().setDisplays),
  ]);
  for (const r of results) {
    if (r.status === 'rejected') console.error('resync failed', r.reason);
  }
}
