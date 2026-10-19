import { describe, expect, it } from 'vitest';
import rustSource from '../../../src-tauri/src/events.rs?raw';
import type { AppEvent } from './events';

// Type-level: a missing or extra key vs the TS union is a typecheck error (gate runs typecheck).
const TS_VARIANTS: Record<AppEvent['type'], true> = {
  BusLag: true,
  Ping: true,
  ConfigReloaded: true,
  SlideUpdate: true,
  PresentationVisibility: true,
  SlideAdvance: true,
  SlideRewind: true,
  DeckEnd: true,
  DeckStart: true,
  TimerTick: true,
  TimerReset: true,
  DeckImportProgress: true,
  DeckImportComplete: true,
  DeckImportFailed: true,
  SceneMediaUpdate: true,
  SceneSourceChanged: true,
  DisplaysChanged: true,
  DisplayAssigned: true,
  DisplayLost: true,
  PipelineStateChanged: true,
  PipelineError: true,
  PipelineWarning: true,
  PipelineLatency: true,
  PipelineQos: true,
};

const IDENT = /^[A-Za-z_]\w*$/;

// Blanks string literals and drops comments in one pass so quotes and comment markers can't confuse each other.
function clean(src: string): string {
  let out = '';
  for (let i = 0; i < src.length; i++) {
    const two = src.slice(i, i + 2);
    if (two === '//') {
      while (i < src.length && src[i] !== '\n') i++;
      out += '\n';
    } else if (two === '/*') {
      const end = src.indexOf('*/', i + 2);
      if (end < 0) throw new Error('unterminated block comment');
      i = end + 1;
    } else if (src[i] === '"') {
      i++;
      while (i < src.length && src[i] !== '"') i += src[i] === '\\' ? 2 : 1;
      if (i >= src.length) throw new Error('unterminated string literal');
      out += '""';
    } else {
      out += src[i];
    }
  }
  return out;
}

function enumBody(src: string): string {
  const c = clean(src);
  const m = /pub enum AppEvent\b/.exec(c);
  if (!m) throw new Error('pub enum AppEvent not found');
  const open = c.indexOf('{', m.index);
  let depth = 0;
  for (let i = open; i < c.length; i++) {
    if ('{(['.includes(c[i])) depth++;
    else if ('})]'.includes(c[i]) && --depth === 0) return c.slice(open + 1, i);
  }
  throw new Error('AppEvent enum braces are unbalanced');
}

// Top-level variant names; throws on anything that is not a bare identifier.
function parseRustVariants(src: string): string[] {
  const body = enumBody(src);
  const names: string[] = [];
  let depth = 0;
  let token = '';
  const flush = () => {
    const t = token.trim();
    token = '';
    if (!t) return;
    if (!IDENT.test(t)) throw new Error('unsupported variant syntax: ' + t);
    names.push(t);
  };
  for (let i = 0; i < body.length; i++) {
    const ch = body[i];
    if (depth === 0 && ch === '#') {
      const attr = /^#\s*\[/.exec(body.slice(i));
      if (!attr) throw new Error('stray # in enum body');
      let d = 0;
      for (i += attr[0].length - 1; i < body.length; i++) {
        if (body[i] === '[') d++;
        else if (body[i] === ']' && --d === 0) break;
      }
      if (d !== 0) throw new Error('unterminated attribute');
    } else if ('{(['.includes(ch)) {
      if (depth === 0) flush();
      depth++;
    } else if ('})]'.includes(ch)) {
      depth--;
    } else if (depth === 0 && ch === ',') {
      flush();
    } else if (depth === 0) {
      token += ch;
    }
  }
  flush();
  return names;
}

// Independent count: collapse nested bodies and attributes, then count comma-separated segments.
function countVariantsByReduction(src: string): number {
  let b = enumBody(src).replace(/#\s*\[[^\]]*\]/g, '');
  for (let prev = ''; prev !== b; ) {
    prev = b;
    b = b.replace(/\{[^{}]*\}|\([^()]*\)/g, '');
  }
  return b.split(',').filter((s) => s.trim() !== '').length;
}

const rust = parseRustVariants(rustSource);
const modelled = Object.keys(TS_VARIANTS);

describe('AppEvent union vs events.rs', () => {
  it('parser handles unit, tuple, struct variants, attributes and comments', () => {
    const src = 'pub enum AppEvent {\n #[serde(x)] A,\n // c\n B { x: Vec<u8> },\n C(Vec<Foo>),\n D }';
    expect(parseRustVariants(src)).toEqual(['A', 'B', 'C', 'D']);
    expect(countVariantsByReduction(src)).toBe(4);
  });

  it('parser ignores // and } inside attribute strings', () => {
    const src = 'pub enum AppEvent {\n #[doc = "a // b } c"]\n A,\n B,\n}';
    expect(parseRustVariants(src)).toEqual(['A', 'B']);
  });

  it('parser rejects explicit discriminants and raw identifiers', () => {
    expect(() => parseRustVariants('pub enum AppEvent { A = 1, B }')).toThrow(/unsupported variant/);
    expect(() => parseRustVariants('pub enum AppEvent { r#Type, B }')).toThrow(/stray #/);
  });

  it('parser rejects unbalanced or missing enums', () => {
    expect(() => parseRustVariants('pub enum AppEvent { A, B {')).toThrow(/unbalanced/);
    expect(() => parseRustVariants('pub enum AppEventX { A }')).toThrow(/not found/);
  });

  it('parses every variant of the real enum (structural count cross-check)', () => {
    expect(rust.length).toBeGreaterThan(0);
    expect(rust.length).toBe(countVariantsByReduction(rustSource));
    expect(new Set(rust).size).toBe(rust.length);
  });

  it('the TS union has no variant Rust does not emit', () => {
    expect(modelled.filter((v) => !rust.includes(v))).toEqual([]);
  });

  it('every Rust variant is in the TS union', () => {
    expect(rust.filter((v) => !modelled.includes(v)), 'add to the TS AppEvent union and TS_VARIANTS').toEqual([]);
  });
});
