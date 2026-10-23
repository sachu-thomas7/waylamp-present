import { afterEach, describe, expect, it, vi } from 'vitest';
import { mediaUrl } from './mediaUrl';

describe('mediaUrl', () => {
  afterEach(() => vi.unstubAllGlobals());

  it('uses the http://sdmedia.localhost form on Windows and encodes the whole path', () => {
    vi.stubGlobal('navigator', { userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64)' });
    expect(mediaUrl(String.raw`C:\Users\a b\x.png`)).toBe('http://sdmedia.localhost/C%3A%5CUsers%5Ca%20b%5Cx.png');
  });

  it('uses the sdmedia:// form elsewhere', () => {
    vi.stubGlobal('navigator', { userAgent: 'Mozilla/5.0 (Macintosh)' });
    expect(mediaUrl('/data/a#b.png')).toBe('sdmedia://localhost/%2Fdata%2Fa%23b.png');
  });
});
