import { useEffect, useState } from 'react';
import { onAppEvent } from '../lib/events';
import {
  PLATFORM_DEFAULT_URL, streamRemoveDestination, streamSaveDestination, streamSetDestinationEnabled,
  streamSetSettings, streamStart, streamStop,
  type DestinationState, type StreamDestination, type StreamPlatform, type StreamSettings,
} from '../lib/ipc';
import { refreshStream, useStreamStore } from '../store/streamStore';
import { Badge } from './ui/Badge';
import { Button } from './ui/Button';
import { Card } from './ui/Card';
import { SegmentedControl } from './ui/SegmentedControl';

const muted = { fontSize: 11.5, color: 'var(--color-text-muted)' } as const;
const field = {
  padding: '6px 8px', borderRadius: 'var(--radius-sm)', border: '1px solid var(--color-border)',
  background: 'var(--color-bg)', color: 'var(--color-text)', fontSize: 12, minWidth: 0,
} as const;

type Quality = '720p' | '1080p';
const QUALITY: Record<Quality, Pick<StreamSettings, 'width' | 'height' | 'video_kbps'>> = {
  '720p': { width: 1280, height: 720, video_kbps: 3000 },
  '1080p': { width: 1920, height: 1080, video_kbps: 6000 },
};
const PLATFORM_LABEL: Record<StreamPlatform, string> = { facebook: 'Facebook', youtube: 'YouTube', custom: 'Custom RTMP' };

function errorMessage(e: unknown): string {
  if (typeof e === 'object' && e !== null && 'message' in e) return String(e.message);
  return String(e);
}

export function formatElapsed(secs: number): string {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  return [h, m, s].map((n) => String(n).padStart(2, '0')).join(':');
}

function healthColor(state: DestinationState['state']): string {
  if (state === 'Live') return 'var(--color-ok)';
  if (state === 'Failed') return 'var(--color-bad)';
  if (state === 'Idle') return 'var(--color-text-muted)';
  return 'var(--color-warn)';
}

function stateLabel(s: DestinationState): string {
  switch (s.state) {
    case 'Live': return 'Live';
    case 'Idle': return 'Off';
    case 'Connecting': return 'Connecting…';
    case 'Reconnecting': return `Reconnecting (try ${s.detail.attempt}, in ${s.detail.retry_in_secs}s)`;
    case 'Failed': return `Failed: ${s.detail}`;
  }
}

function slug(name: string): string {
  return name.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '').slice(0, 24) || 'dest';
}

export function StreamPanel() {
  const { config, status, lastError, setLastError, patchHealth } = useStreamStore();
  const [confirming, setConfirming] = useState<'start' | 'stop' | null>(null);
  const [adding, setAdding] = useState(false);
  const [form, setForm] = useState<{ platform: StreamPlatform; name: string; url: string; key: string }>({
    platform: 'youtube', name: '', url: PLATFORM_DEFAULT_URL.youtube, key: '',
  });
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    refreshStream().catch((e) => setLastError(errorMessage(e)));
    const unlisten = onAppEvent((event) => {
      if (event.type === 'StreamStateChanged' || event.type === 'StreamDestinationChanged') {
        refreshStream().catch((e) => setLastError(errorMessage(e)));
      } else if (event.type === 'StreamHealth') {
        patchHealth(event.payload.id, event.payload.bitrate_kbps, event.payload.dropped_frames);
      }
    });
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  const live = status?.state === 'Live' || status?.state === 'Starting';
  useEffect(() => {
    if (!live) return;
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, [live]);

  async function run(action: () => Promise<unknown>) {
    setLastError(null);
    try {
      await action();
      await refreshStream();
    } catch (e) {
      setLastError(errorMessage(e));
    }
  }

  function pickPlatform(platform: StreamPlatform) {
    setForm((f) => ({ ...f, platform, url: PLATFORM_DEFAULT_URL[platform] }));
  }

  function saveForm() {
    const existing = new Set(config?.destinations.map((d) => d.id) ?? []);
    let id = slug(form.name || PLATFORM_LABEL[form.platform]);
    for (let i = 2; existing.has(id); i++) id = `${slug(form.name || form.platform)}-${i}`;
    const destination: StreamDestination = {
      id, name: form.name.trim() || PLATFORM_LABEL[form.platform], platform: form.platform,
      ingest_url: form.url.trim(), enabled: true,
    };
    run(async () => {
      await streamSaveDestination(destination, form.key.trim() || null);
      setAdding(false);
      setForm({ platform: 'youtube', name: '', url: PLATFORM_DEFAULT_URL.youtube, key: '' });
    });
  }

  const quality: Quality = config && config.settings.height <= 720 ? '720p' : '1080p';
  const enabledCount = config?.destinations.filter((d) => d.enabled).length ?? 0;
  const missingKey = config?.destinations.some((d) => d.enabled && !d.key_set) ?? false;
  const elapsed = status?.started_unix_ms ? Math.max(0, Math.floor((now - status.started_unix_ms) / 1000)) : 0;
  const stateOf = (id: string) => status?.destinations.find((d) => d.id === id);

  return (
    <Card
      title="Live Stream"
      headerRight={
        live
          ? <Badge tone="warn">LIVE {formatElapsed(elapsed)}</Badge>
          : <Badge>Off</Badge>
      }
    >
      {lastError && <p style={{ ...muted, color: 'var(--color-warn)', margin: 0 }}>{lastError}</p>}
      {live && status?.encoder && (
        <span style={muted}>Encoder: {status.encoder}{status.hardware ? ' (hardware)' : ' (software)'}</span>
      )}

      {config?.destinations.length === 0 && !adding && (
        <span style={muted}>No destinations yet. Add Facebook, YouTube or any RTMP server with its stream key.</span>
      )}

      {config?.destinations.map((d) => {
        const st = stateOf(d.id);
        const ds: DestinationState = st?.state ?? { state: 'Idle' };
        return (
          <div key={d.id} data-testid={`stream-dest-${d.id}`} style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 8 }}>
              <span style={{ display: 'flex', alignItems: 'center', gap: 8, minWidth: 0 }}>
                <span
                  aria-hidden="true"
                  style={{ width: 9, height: 9, borderRadius: '50%', background: healthColor(ds.state), flexShrink: 0 }}
                />
                <span style={{ fontSize: 13, fontWeight: 600, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{d.name}</span>
                <Badge>{PLATFORM_LABEL[d.platform]}</Badge>
                {!d.key_set && <Badge tone="warn">No key</Badge>}
              </span>
              <span style={{ display: 'flex', gap: 6 }}>
                <Button
                  variant="secondary"
                  onClick={() => run(() => streamSetDestinationEnabled(d.id, !d.enabled))}
                  aria-label={d.enabled ? `Mute ${d.name}` : `Unmute ${d.name}`}
                >
                  {d.enabled ? 'Mute' : 'Unmute'}
                </Button>
                <Button variant="secondary" onClick={() => run(() => streamRemoveDestination(d.id))} aria-label={`Remove ${d.name}`}>
                  Remove
                </Button>
              </span>
            </div>
            <span style={{ ...muted, color: ds.state === 'Failed' ? 'var(--color-bad)' : 'var(--color-text-muted)' }}>
              {d.enabled ? stateLabel(ds) : 'Muted'}
              {live && st && ds.state === 'Live' && ` · ${st.bitrate_kbps} kbps · ${st.dropped_frames} dropped`}
            </span>
          </div>
        );
      })}

      {adding ? (
        <div role="group" aria-label="Add destination" style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
          <SegmentedControl<StreamPlatform>
            options={[{ value: 'youtube', label: 'YouTube' }, { value: 'facebook', label: 'Facebook' }, { value: 'custom', label: 'Custom' }]}
            value={form.platform}
            onChange={pickPlatform}
          />
          <input style={field} aria-label="Destination name" placeholder="Name (e.g. Sunday service)" value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} />
          <input style={field} aria-label="Ingest URL" placeholder="rtmp(s):// ingest URL" value={form.url} onChange={(e) => setForm({ ...form, url: e.target.value })} />
          <input style={field} type="password" aria-label="Stream key" placeholder="Stream key (stored in the keychain)" value={form.key} onChange={(e) => setForm({ ...form, key: e.target.value })} autoComplete="off" />
          <div style={{ display: 'flex', gap: 6 }}>
            <Button variant="primary" disabled={!form.url.trim() || !form.key.trim()} onClick={saveForm}>Save destination</Button>
            <Button variant="secondary" onClick={() => { setAdding(false); setForm({ ...form, key: '' }); }}>Cancel</Button>
          </div>
        </div>
      ) : (
        <Button variant="secondary" onClick={() => setAdding(true)} disabled={(config?.destinations.length ?? 0) >= 4}>Add destination</Button>
      )}

      {config && (
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
          <span style={muted}>Quality</span>
          <SegmentedControl<Quality>
            options={[{ value: '720p', label: '720p · 3 Mbps' }, { value: '1080p', label: '1080p · 6 Mbps' }]}
            value={quality}
            onChange={(q) => !live && run(() => streamSetSettings({ ...config.settings, ...QUALITY[q] }))}
          />
        </div>
      )}

      {!live && !confirming && (
        <Button variant="live" pill disabled={enabledCount === 0 || missingKey} onClick={() => setConfirming('start')}>
          Go Live
        </Button>
      )}
      {!live && enabledCount > 0 && missingKey && <span style={muted}>Every enabled destination needs a stream key.</span>}
      {live && !confirming && (
        <Button variant="secondary" pill onClick={() => setConfirming('stop')}>End Stream</Button>
      )}
      {confirming && (
        <div role="alertdialog" aria-label={confirming === 'start' ? 'Confirm go live' : 'Confirm end stream'} style={{ display: 'flex', flexDirection: 'column', gap: 8, padding: 12, border: '1px solid var(--color-warn)', borderRadius: 'var(--radius-sm)' }}>
          <span style={{ fontSize: 13, fontWeight: 600 }}>
            {confirming === 'start'
              ? `Go live now to ${enabledCount} ${enabledCount === 1 ? 'destination' : 'destinations'}?`
              : 'End the stream for every destination?'}
          </span>
          <div style={{ display: 'flex', gap: 8 }}>
            <Button
              variant={confirming === 'start' ? 'live' : 'primary'}
              onClick={() => {
                const which = confirming;
                setConfirming(null);
                run(which === 'start' ? streamStart : streamStop);
              }}
            >
              {confirming === 'start' ? 'Yes, go live' : 'Yes, end stream'}
            </Button>
            <Button variant="secondary" onClick={() => setConfirming(null)}>Cancel</Button>
          </div>
        </div>
      )}
    </Card>
  );
}
