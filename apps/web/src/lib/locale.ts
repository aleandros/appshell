import { z } from 'zod';

const languageTag = z.string().refine((value) => {
  try {
    return /^[a-zA-Z0-9-]+$/.test(value) && Intl.getCanonicalLocales(value)[0] === value;
  } catch {
    return false;
  }
}, 'Use a canonical BCP 47 language tag, for example en or pt-BR.');
export const localeConfigSchema = z
  .object({
    defaultLanguage: languageTag,
    languages: z
      .array(
        z
          .object({
            tag: languageTag,
            label: z.string().min(1),
            direction: z.enum(['ltr', 'rtl']),
          })
          .strict(),
      )
      .min(1),
    checkMissingTranslations: z.boolean(),
  })
  .strict()
  .refine(
    (value) =>
      new Set(value.languages.map(({ tag }) => tag)).size === value.languages.length &&
      value.languages.some(({ tag }) => tag === value.defaultLanguage),
    'Default language must be supported and language tags must be unique.',
  );
export type LocaleConfig = z.infer<typeof localeConfigSchema>;
export const catalogSchema = z.record(z.string(), z.string());
export const languageStorageKey = 'appshell-language';

/** BCP 47 lookup: ordered preferences, then progressively less specific tags. */
export function matchLanguage(
  preferences: readonly string[],
  supported: readonly string[],
): string | undefined {
  for (const preference of preferences) {
    let tag: string;
    try {
      tag = Intl.getCanonicalLocales(preference)[0] ?? '';
    } catch {
      continue;
    }
    while (tag) {
      const exact = supported.find((candidate) => candidate.toLowerCase() === tag.toLowerCase());
      if (exact) return exact;
      const variant = supported.find((candidate) =>
        candidate.toLowerCase().startsWith(`${tag.toLowerCase()}-`),
      );
      if (variant) return variant;
      tag = tag.replace(/-?[^-]+$/, '').replace(/-[a-z0-9]$/i, '');
    }
  }
  return undefined;
}

export function detectLanguage(
  config: LocaleConfig,
  preferences: readonly string[],
  saved: string | null,
): string {
  const supported = config.languages.map(({ tag }) => tag);
  return (
    matchLanguage(saved ? [saved] : [], supported) ??
    matchLanguage(preferences, supported) ??
    config.defaultLanguage
  );
}
