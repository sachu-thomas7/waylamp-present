import type { SceneMediaPayload } from '../lib/ipc';

interface Props {
  media: SceneMediaPayload;
}

export function SceneMediaView({ media }: Props) {
  const style: React.CSSProperties = {
    width: '100%',
    height: '100%',
    objectFit: 'contain',
    display: 'block',
    background: 'black',
  };

  if (media.media_type === 'Image') {
    return <img src={media.path} alt="" style={style} />;
  }

  return (
    <video
      src={media.path}
      autoPlay
      loop
      style={style}
    />
  );
}