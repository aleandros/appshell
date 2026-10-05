import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createLocalization } from './i18n-runtime';
import { languageStorageKey, localeConfigSchema } from './locale';
const config = localeConfigSchema.parse({
  defaultLanguage: 'en',
  languages: [
    { tag: 'en', label: 'English', direction: 'ltr' },
    { tag: 'es', label: 'Español', direction: 'ltr' },
    { tag: 'ar', label: 'العربية', direction: 'rtl' },
  ],
  checkMissingTranslations: false,
});
const english = {
  Hello: 'Hello',
  Fallback: 'Fallback',
  people_one: '{{count}} person',
  people_other: '{{count}} people',
  Greeting: 'Hello {{name}}',
};
let storage: Map<string, string>;
beforeEach(() => {
  storage = new Map();
  vi.stubGlobal('localStorage', {
    getItem: (key: string) => storage.get(key) ?? null,
    setItem: (key: string, value: string) => storage.set(key, value),
  });
  vi.stubGlobal('navigator', { languages: ['es-MX', 'en'] });
  vi.stubGlobal('document', { documentElement: { lang: '', dir: '' } });
});
afterEach(() => vi.unstubAllGlobals());
describe('localization runtime', () => {
  it('loads only the default and selected catalogs, falls back and caches switches', async () => {
    const es = vi.fn().mockResolvedValue({ Hello: 'Hola', Fallback: '' });
    const ar = vi.fn().mockResolvedValue({ Hello: 'مرحبا' });
    const app = createLocalization(config, { en: () => Promise.resolve(english), es, ar });
    await app.initialize();
    expect(app.i18n.t('Hello')).toBe('Hola');
    expect(app.i18n.t('Fallback')).toBe('Fallback');
    expect(ar).not.toHaveBeenCalled();
    expect(storage.has(languageStorageKey)).toBe(false);
    await app.selectLanguage('ar');
    expect(document.documentElement).toEqual({ lang: 'ar', dir: 'rtl' });
    expect(storage.get(languageStorageKey)).toBe('ar');
    await app.selectLanguage('es');
    expect(document.documentElement.dir).toBe('ltr');
    expect(es).toHaveBeenCalledTimes(1);
    await expect(app.selectLanguage('fr')).rejects.toThrow('Unsupported language');
  });
  it('restores explicit preference without loading the browser language', async () => {
    storage.set(languageStorageKey, 'en');
    const es = vi.fn();
    const app = createLocalization(config, { en: () => Promise.resolve(english), es });
    await app.initialize();
    expect(app.i18n.language).toBe('en');
    expect(es).not.toHaveBeenCalled();
    expect(app.i18n.t('people', { count: 1 })).toBe('1 person');
    expect(app.i18n.t('people', { count: 2 })).toBe('2 people');
  });
  it('keeps the current language on failed switches and allows retry', async () => {
    const es = vi
      .fn()
      .mockRejectedValueOnce(new Error('offline'))
      .mockResolvedValue({ Hello: 'Hola' });
    const app = createLocalization(config, {
      en: () => Promise.resolve(english),
      es,
      ar: () => Promise.resolve({ Hello: 42 }),
    });
    await app.initialize();
    expect(app.i18n.language).toBe('en');
    await expect(app.selectLanguage('ar')).rejects.toThrow();
    expect(app.i18n.language).toBe('en');
    expect(storage.size).toBe(0);
    await app.selectLanguage('es');
    expect(app.i18n.t('Hello')).toBe('Hola');
  });
  it('works when storage is blocked and never persists automatic detection', async () => {
    vi.stubGlobal('localStorage', {
      getItem: () => {
        throw new Error('blocked');
      },
      setItem: () => {
        throw new Error('blocked');
      },
    });
    const app = createLocalization(config, {
      en: () => Promise.resolve(english),
      es: () => Promise.resolve({ Hello: 'Hola' }),
    });
    await app.initialize();
    await app.selectLanguage('en');
    expect(app.i18n.language).toBe('en');
  });
  it('ignores stale language loads when a newer selection finishes first', async () => {
    storage.set(languageStorageKey, 'en');
    let complete: ((value: unknown) => void) | undefined;
    const app = createLocalization(config, {
      en: () => Promise.resolve(english),
      es: () =>
        new Promise((resolve) => {
          complete = resolve;
        }),
      ar: () => Promise.resolve({ Hello: 'مرحبا' }),
    });
    await app.initialize();
    const slow = app.selectLanguage('es');
    await app.selectLanguage('ar');
    complete?.({ Hello: 'Hola' });
    await slow;
    expect(app.i18n.language).toBe('ar');
    expect(storage.get(languageStorageKey)).toBe('ar');
  });
});
