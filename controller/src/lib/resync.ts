import {
  compositorGetState, deckGetAllSlides, displayList, ndiListSources, pipelineGetState, sceneGetActiveSource,
  slideGetCurrent,
} from './ipc';
import { useCompositorStore } from '../store/compositorStore';
import { useDisplayStore } from '../store/displayStore';
import { useNdiStore } from '../store/ndiStore';
import { usePipelineStore } from '../store/pipelineStore';
import { useSceneStore } from '../store/sceneStore';
import { useControllerSlideStore } from '../store/slideStore';
import { refreshWhip } from '../store/whipStore';

// A BusLag means events were dropped, so pull every pull-able piece of state again. Slide, scene and
// presentation state are also re-pushed by the backend right after the lag event.
export async function resyncFromBackend(): Promise<void> {
  const slides = useControllerSlideStore.getState();
  const results = await Promise.allSettled([
    slideGetCurrent().then((s) => { if (s) slides.setSlide(s); }),
    deckGetAllSlides().then(slides.setSlides),
    sceneGetActiveSource().then(useSceneStore.getState().setActiveSource),
    displayList().then(useDisplayStore.getState().setDisplays),
    pipelineGetState().then(usePipelineStore.getState().setState),
    compositorGetState().then(useCompositorStore.getState().setStatus),
    ndiListSources().then(useNdiStore.getState().setSnapshot).then(refreshWhip),
  ]);
  for (const r of results) {
    if (r.status === 'rejected') console.error('resync failed', r.reason);
  }
}
