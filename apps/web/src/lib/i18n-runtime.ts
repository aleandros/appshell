import { createInstance } from 'i18next';
import { initReactI18next } from 'react-i18next';
import { catalogSchema, detectLanguage, languageStorageKey } from './locale';
import type { LocaleConfig } from './locale';

/** A separate instance keeps tests and embedded applications isolated. */
export function createLocalization(
  config: LocaleConfig,
  catalogs: Record<string, () => Promise<unknown>>,
) {
  const i18n = createInstance();
  const pending = new Map<string, Promise<void>>();

  async function loadLanguage(tag: string): Promise<void> {
    if (i18n.hasResourceBundle(tag, 'translation')) return;
    const existing = pending.get(tag);
    if (existing) return existing;
    const load = catalogs[tag];
    if (!load) throw new Error(`Missing translation catalog: ${tag}`);
    const request = load()
      .then((data) => {
        i18n.addResourceBundle(tag, 'translation', catalogSchema.parse(data));
      })
      .finally(() => pending.delete(tag));
    pending.set(tag, request);
    return request;
  }

  function updateDocument(tag: string) {
    document.documentElement.lang = tag;
    document.documentElement.dir =
      config.languages.find((language) => language.tag === tag)?.direction ?? 'ltr';
  }

  async function initialize(): Promise<void> {
    await i18n.use(initReactI18next).init({
      lng: config.defaultLanguage,
      fallbackLng: config.defaultLanguage,
      supportedLngs: config.languages.map(({ tag }) => tag),
      resources: {
        [config.defaultLanguage]: {
          translation: catalogSchema.parse(await catalogs[config.defaultLanguage]?.()),
        },
      },
      keySeparator: false,
      nsSeparator: false,
      returnEmptyString: false,
      interpolation: { escapeValue: false }, // React escapes text at the rendering boundary.
      react: { useSuspense: false },
    });
    let saved: string | null = null;
    try {
      saved = localStorage.getItem(languageStorageKey);
    } catch {
      /* Storage is optional. */
    }
    const language = detectLanguage(config, navigator.languages, saved);
    try {
      await loadLanguage(language);
      await i18n.changeLanguage(language);
    } catch {
      await i18n.changeLanguage(config.defaultLanguage);
    }
    updateDocument(i18n.language);
    i18n.on('languageChanged', updateDocument);
  }

  let selection = 0;
  async function selectLanguage(tag: string): Promise<void> {
    if (!config.languages.some((language) => language.tag === tag)) {
      throw new Error('Unsupported language');
    }
    const request = ++selection;
    await loadLanguage(tag);
    if (request !== selection) return;
    await i18n.changeLanguage(tag);
    try {
      localStorage.setItem(languageStorageKey, tag);
    } catch {
      /* Still works without storage. */
    }
  }

  return { i18n, initialize, selectLanguage };
}
