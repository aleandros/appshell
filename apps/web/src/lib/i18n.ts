import settings from '../config/i18n.json';
import english from '../config/locales/en.json';
import { localeConfigSchema } from './locale';
import { createLocalization } from './i18n-runtime';

export const localeConfig = localeConfigSchema.parse(settings);
const modules = import.meta.glob(['../config/locales/*.json', '!../config/locales/en.json'], {
  import: 'default',
});
const catalogs: Record<string, () => Promise<unknown>> = {};
for (const [path, load] of Object.entries(modules)) {
  const tag = path
    .split('/')
    .pop()
    ?.replace(/\.json$/, '');
  if (tag) catalogs[tag] = load;
}
// The default English setup needs no extra request. Other catalogs remain lazy chunks.
catalogs.en = () => Promise.resolve(english);
const localization = createLocalization(localeConfig, catalogs);
export const i18n = localization.i18n;
export const initializeI18n = localization.initialize;
export const selectLanguage = localization.selectLanguage;
