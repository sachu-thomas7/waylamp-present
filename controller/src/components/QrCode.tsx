import type { QrMatrix } from '../lib/ipc';

const QUIET = 4;

export function QrCode({ matrix, label }: { matrix: QrMatrix; label: string }) {
  const { size, modules } = matrix;
  let path = '';
  modules.forEach((dark, i) => {
    if (dark) path += `M${(i % size) + QUIET} ${Math.floor(i / size) + QUIET}h1v1h-1z`;
  });
  const span = size + QUIET * 2;
  return (
    <svg role="img" aria-label={label} viewBox={`0 0 ${span} ${span}`} width={132} height={132} shapeRendering="crispEdges">
      <rect width={span} height={span} fill="#fff" />
      <path d={path} fill="#000" />
    </svg>
  );
}
