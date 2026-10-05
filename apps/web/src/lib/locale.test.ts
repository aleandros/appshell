import { describe, expect, it } from 'vitest';
import { detectLanguage, localeConfigSchema, matchLanguage } from './locale';
const config = localeConfigSchema.parse({
  defaultLanguage: 'en',
  languages: [
    { tag: 'en', label: 'English', direction: 'ltr' },
    { tag: 'es', label: 'Español', direction: 'ltr' },
  ],
  checkMissingTranslations: false,
});
describe('locale negotiation', () => {
  it('prefers saved choice, then ordered browser preferences, then default', () => {
    expect(detectLanguage(config, ['es-MX', 'en-US'], 'en')).toBe('en');
    expect(detectLanguage(config, ['fr', 'es-MX', 'en-US'], null)).toBe('es');
    expect(detectLanguage(config, ['de'], null)).toBe('en');
    expect(detectLanguage(config, ['es'], 'removed-locale')).toBe('es');
  });
  it('handles canonical case, script and region tags, extensions and invalid input', () => {
    expect(matchLanguage(['ES-mx'], ['es-MX', 'es'])).toBe('es-MX');
    expect(matchLanguage(['zh-Hant-TW'], ['zh', 'zh-Hant'])).toBe('zh-Hant');
    expect(matchLanguage(['en-GB'], ['es', 'en-US'])).toBe('en-US');
    expect(matchLanguage(['de-DE-u-co-phonebk'], ['de'])).toBe('de');
    expect(matchLanguage(['../../en', '', 'es_MX', 'es'], ['en', 'es'])).toBe('es');
  });
  it('rejects invalid, duplicate and unsupported defaults at configuration boundary', () => {
    expect(localeConfigSchema.safeParse({ ...config, defaultLanguage: 'fr' }).success).toBe(false);
    expect(
      localeConfigSchema.safeParse({
        ...config,
        languages: [...config.languages, config.languages[0]],
      }).success,
    ).toBe(false);
    expect(localeConfigSchema.safeParse({ ...config, languages: [] }).success).toBe(false);
  });
});
