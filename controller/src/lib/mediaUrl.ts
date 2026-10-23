// Webviews cannot load file paths; Rust serves them through the `sdmedia` scheme (media_protocol.rs).
const base = (): string =>
  /windows/i.test(globalThis.navigator?.userAgent ?? '') ? 'http://sdmedia.localhost' : 'sdmedia://localhost';

export function mediaUrl(path: string): string {
  return `${base()}/${encodeURIComponent(path)}`;
}
