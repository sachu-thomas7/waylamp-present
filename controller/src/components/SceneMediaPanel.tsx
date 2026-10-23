import { useEffect } from 'react';
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
import { mediaUrl } from '../lib/mediaUrl';
import { Card } from './ui/Card';
import { Button } from './ui/Button';
import { SegmentedControl } from './ui/SegmentedControl';
import { IconImage, IconVideo } from './ui/icons';

export function SceneMediaPanel() {
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
    <Card title="Scene Media">
      <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
        <SegmentedControl
          value={activeSource ?? 'SlideDeck'}
          onChange={handleSourceChange}
          options={[
            { value: 'SlideDeck', label: 'Slide Deck' },
            { value: 'SceneMedia', label: 'Scene Media' },
          ]}
        />
        <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
          <span style={{ width: 6, height: 6, borderRadius: '50%', background: 'var(--color-live)', flexShrink: 0 }} />
          <span style={{ fontSize: 11.5, color: 'var(--color-text-muted)' }}>
            On screen now: <strong style={{ color: 'var(--color-text)' }}>{activeSource === 'SceneMedia' ? 'Scene Media' : 'Slide Deck'}</strong>
          </span>
        </div>
      </div>

      <div style={{ display: 'flex', gap: 8 }}>
        <Button variant="secondary" icon={<IconImage />} onClick={handlePushImage} style={{ flex: 1 }}>
          Push Image
        </Button>
        <Button variant="secondary" icon={<IconVideo />} onClick={handlePushVideo} style={{ flex: 1 }}>
          Push Video
        </Button>
      </div>
      <span style={{ fontSize: 11, color: 'var(--color-text-muted)', marginTop: -8 }}>
        Pushing stages the file — switch to "Scene Media" above to put it on screen.
      </span>

      <div>
        {currentMedia ? (
          <>
            <span style={{ fontSize: 11.5, color: 'var(--color-text-muted)' }}>Staged: {currentMedia.display_name}</span>
            {currentMedia.media_type === 'Image' && (
              <img
                src={mediaUrl(currentMedia.path)}
                alt={currentMedia.display_name}
                style={{ maxWidth: '100%', maxHeight: 120, display: 'block', marginTop: 8, borderRadius: 'var(--radius-sm)' }}
              />
            )}
          </>
        ) : (
          <span style={{ fontSize: 11.5, color: 'var(--color-text-muted)' }}>Staged: none</span>
        )}
      </div>
    </Card>
  );
}
