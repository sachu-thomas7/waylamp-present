import { useEffect, useState } from 'react';
import { onAppEvent } from '../lib/events';
import { whipGetQr, whipSetEnabled, whipSetToken, type WhipLinkState } from '../lib/ipc';
import { refreshWhip, useWhipStore } from '../store/whipStore';
import { Badge } from './ui/Badge';
import { Button } from './ui/Button';
import { QrCode } from './QrCode';

const heading = { fontSize: 11.5, fontWeight: 600, letterSpacing: '0.07em', textTransform: 'uppercase', color: 'var(--color-text-muted)' } as const;
const row = { display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 8 } as const;
const note = { fontSize: 11.5, margin: 0 } as const;
const mono = { fontFamily: 'var(--font-mono)', fontSize: 10.5, wordBreak: 'break-all', color: 'var(--color-text-muted)' } as const;

const STATE_LABEL: Record<WhipLinkState, string> = {
  Connected: 'Connected',
  Disconnected: 'Reconnecting…',
  Lost: 'Source Lost',
};

export function WhipPanel() {
  const { info, qr, lastError, setQr, setLastError } = useWhipStore();
  const [token, setToken] = useState('');
  const [qrSlot, setQrSlot] = useState<string | null>(null);

  useEffect(() => {
    const report = (e: unknown) => setLastError(JSON.stringify(e));
    refreshWhip().catch(report);
    const unlisten = onAppEvent((event) => {
      if (event.type === 'WhipServerChanged' || event.type === 'WhipSessionChanged' || event.type === 'SourceRemoved') {
        refreshWhip().catch(report);
      }
    });
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  async function run(action: () => Promise<void>) {
    setLastError(null);
    try {
      await action();
      await refreshWhip();
    } catch (e) {
      setLastError(JSON.stringify(e));
    }
  }

  async function showQr(slot: string) {
    if (qrSlot === slot) return setQrSlot(null);
    setLastError(null);
    try {
      setQr(slot, await whipGetQr(slot));
      setQrSlot(slot);
    } catch (e) {
      setLastError(JSON.stringify(e));
    }
  }

  const running = info?.running ?? false;

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
      <div style={row}>
        <span style={heading}>Phone Cameras</span>
        <span style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
          <Badge>{running ? `Listening :${info?.port}` : 'Off'}</Badge>
          <Button
            variant={running ? 'secondary' : 'primary'}
            disabled={!info}
            onClick={() => run(() => whipSetEnabled(!running))}
            aria-label={running ? 'Disable phone cameras' : 'Enable phone cameras'}
          >
            {running ? 'Disable' : 'Enable'}
          </Button>
        </span>
      </div>
      {info?.unavailable && <p style={{ ...note, color: 'var(--color-warn)' }}>{info.unavailable}</p>}
      {info?.last_error && !running && <p style={{ ...note, color: 'var(--color-warn)' }}>{info.last_error}</p>}
      {lastError && <p style={{ ...note, color: 'var(--color-warn)' }}>{lastError}</p>}
      {running && info && !info.token_set && (
        <p style={{ ...note, color: 'var(--color-warn)' }}>
          No access token set — anyone on this network can push video here.
        </p>
      )}
      {running && info && !info.host && (
        <p style={{ ...note, color: 'var(--color-warn)' }}>No LAN address found — connect to a network.</p>
      )}

      {running && info?.slots.map(({ slot, url, session }) => (
        <div key={slot} style={{ display: 'flex', flexDirection: 'column', gap: 4 }} data-testid={`whip-slot-${slot}`}>
          <div style={row}>
            <span style={{ fontSize: 12.5 }}>{slot}</span>
            <span style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
              {session && <Badge tone={session.state === 'Connected' ? 'neutral' : 'warn'}>{STATE_LABEL[session.state]}</Badge>}
              <Button variant="secondary" disabled={!url} onClick={() => showQr(slot)} aria-label={`QR code for ${slot}`}>QR</Button>
            </span>
          </div>
          <span style={mono}>{url ?? '—'}</span>
          {qrSlot === slot && qr[slot] && <QrCode matrix={qr[slot]} label={`QR code for ${url}`} />}
        </div>
      ))}

      {info && (
        <div style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
          <input
            type="password"
            value={token}
            onChange={(e) => setToken(e.target.value)}
            placeholder={info.token_set ? 'Token set (stored in keychain)' : 'Access token (optional)'}
            aria-label="Access token"
            style={{ flex: 1, minWidth: 0, padding: '6px 8px', borderRadius: 'var(--radius-sm)', border: '1px solid var(--color-border)', background: 'var(--color-bg)', color: 'var(--color-text)', fontSize: 12 }}
          />
          <Button variant="secondary" disabled={!token.trim()} onClick={() => run(async () => { await whipSetToken(token); setToken(''); })}>Save</Button>
          {info.token_set && <Button variant="secondary" onClick={() => run(() => whipSetToken(null))}>Clear</Button>}
        </div>
      )}
      {running && <p style={{ ...note, color: 'var(--color-text-muted)' }}>Phones send the token as a Bearer header; the QR encodes the URL only.</p>}
    </div>
  );
}
