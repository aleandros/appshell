import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createInstance } from 'i18next';
import { I18nextProvider } from 'react-i18next';
import { afterEach, expect, it, vi } from 'vitest';
import { LanguageSwitcher } from './language-switcher';
import { localeConfig } from '../lib/i18n';
vi.mock('../lib/i18n', () => ({
  localeConfig: { languages: [{ tag: 'en', label: 'English', direction: 'ltr' }] },
  selectLanguage: vi.fn(),
}));
afterEach(() => {
  localeConfig.languages.splice(1);
});
async function markup() {
  const i18n = createInstance();
  await i18n.init({ lng: 'en', resources: { en: { translation: { Language: 'Language' } } } });
  return renderToStaticMarkup(
    createElement(I18nextProvider, { i18n }, createElement(LanguageSwitcher)),
  );
}
it('renders no language control with one supported language', async () => {
  expect(await markup()).toBe('');
});
it('renders a labeled native select with autonyms only when multiple languages are enabled', async () => {
  localeConfig.languages.push({ tag: 'es', label: 'Español', direction: 'ltr' });
  const html = await markup();
  expect(html).toContain('<select');
  expect(html).toMatch(/<label[^>]+for="([^"]+)"/);
  expect(html).toContain('Language</label>');
  expect(html).toContain('value="es" lang="es">Español');
  expect(html).toContain('selected="">English');
});
