import { useEffect, useState } from 'react';
import { useSceneStore } from '../store/sceneStore';
import { onAppEvent } from '../lib/events';
import {
  sceneGetActiveSource,
  scenePushImage,
  scenePushVideo,
  sceneSetSource,
  SceneSource,
} from '../lib/ipc';
import { open } from '@tauri-apps/plugin-dialog';

export function SceneMediaPanel() {
  const [isCollapsed, setIsCollapsed] = useState(false);
  const { activeSource, currentMedia, setActiveSource, setCurrentMedia } = useSceneStore();

  useEffect(() => {
    sceneGetActiveSource().then(setActiveSource).catch(console.error);

    const unlisten = onAppEvent((event) => {
      if (event.type === 'SceneMediaUpdate') {
        setCurrentMedia(event.payload);
      } else if (event.type === 'SceneSourceChanged') {
        setActiveSource(event.payload.source);
      }
    });

    return () => { unlisten.then((fn) => fn()); };
  }, []);

  async function handlePushImage() {
    const path = await open({
        multiple: false,
        filters: [{ name: 'Image', extensions: ['png', 'jpg', 'jpeg', 'webp'] }],
    });
    if (typeof path === 'string') {
        await scenePushImage(path).catch(console.error);
    }
}

  async function handlePushVideo() {
    const path = await open({
        multiple: false,
        filters: [{ name: 'Video', extensions: ['mp4', 'webm'] }],
    });
    if (typeof path === 'string') {
        await scenePushVideo(path).catch(console.error);
    }
}

  async function handleSourceChange(source: SceneSource) {
    await sceneSetSource(source).catch(console.error);
  }

  return (
    <section>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <span>▼ Scene Media</span>
        <button onClick={() => setIsCollapsed((c) => !c)}>
          {isCollapsed ? '▼ Expand' : '▲ Collapse'}
        </button>
      </div>

      {!isCollapsed && (
        <>
          <div>
            <span>Active Source: </span>
            <label>
              <input
                type="radio"
                name="sceneSource"
                value="SlideDeck"
                checked={activeSource === 'SlideDeck'}
                onChange={() => handleSourceChange('SlideDeck')}
              />
              {' '}Slide Deck
            </label>
            {' '}
            <label>
              <input
                type="radio"
                name="sceneSource"
                value="SceneMedia"
                checked={activeSource === 'SceneMedia'}
                onChange={() => handleSourceChange('SceneMedia')}
              />
              {' '}Scene Media
            </label>
          </div>

          <div>
            <button onClick={handlePushImage}>Push Image</button>
            {' '}
            <button onClick={handlePushVideo}>Push Video</button>
          </div>

          <div>
            {currentMedia ? (
              <>
                <span>Current media: {currentMedia.display_name}</span>
                {currentMedia.media_type === 'Image' && (
                  <img
                    src={currentMedia.path}
                    alt={currentMedia.display_name}
                    style={{ maxWidth: '100%', maxHeight: 120, display: 'block', marginTop: 4 }}
                  />
                )}
              </>
            ) : (
              <span>Current media: none</span>
            )}
          </div>
        </>
      )}
    </section>
  );
}