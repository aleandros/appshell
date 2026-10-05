# Internationalization

AppShell's React application uses i18next and react-i18next. English is the only
enabled language initially, so there is **no language switcher**. Product developers
choose the languages they support in `apps/web/src/config/i18n.json`; no API,
provider, environment variable, or geolocation service is needed.

## Add a language

1. Copy `apps/web/src/config/locales/en.json` to `es.json` in the same directory and
   translate its values. Keep keys and interpolation names unchanged. Catalogs are
   flat JSON objects, one per canonical BCP 47 tag (`es`, `pt-BR`, `zh-Hant`).
2. Register the language using its own name (an autonym), and its text direction:

   ```json
   {
     "defaultLanguage": "en",
     "languages": [
       { "tag": "en", "label": "English", "direction": "ltr" },
       { "tag": "es", "label": "Español", "direction": "ltr" }
     ],
     "checkMissingTranslations": true
   }
   ```

3. Run `npm run check:i18n -- --strict` and `npm run check:web`, then rebuild.

Only English translations ship with the starter. Register a language after adding
its catalog. Removing a language from configuration removes it from selection and
detection; remove its catalog too if it should not be emitted as a build asset.
The configured default must be enabled and have a catalog. Language configuration
is validated at startup and by the check command.

A labeled native select appears beside appearance controls on public, auth,
workspace and admin screens when **two or more** languages are enabled. It uses
shared UI controls, supports the keyboard, and lists only enabled languages.
Language changes preserve component state, including form input. A failed catalog
load keeps the previous language and provides a retryable error. Error/404 routes
also offer language selection.

## Detection and loading

Initial selection uses:

1. An explicit choice saved in local storage (`appshell-language`).
2. The browser's ordered `navigator.languages` preferences.
3. `defaultLanguage`.

Language tags are canonicalized with `Intl.getCanonicalLocales`. Lookup tries the
exact tag, matching variants, and progressively less specific subtags: `es-MX`
can select `es`, and `zh-Hant-TW` can select `zh-Hant`. Invalid and removed
preferences are ignored. The configured order breaks ties between variants.
A single-language app always selects its configured language.

Browser preferences include region/script information; IP addresses and physical
location are not used. Only explicit choices are saved. If storage is blocked,
selection still works for the current page. The preference is per browser origin,
not an account setting or a cross-device preference; clear that storage key to
return to browser detection. Browser preference changes apply on the next load.

English is bundled with the application. Additional catalogs use Vite's dynamic
imports: only the default and selected catalog are loaded at startup, and later
selections are cached. Catalog loading completes before switching language, so
users do not see translation keys or an intermediate language during a switch.
Missing/empty translations fall back to the configured default language. A failed
initial preferred-language load also falls back to that default. An invalid or
unavailable default catalog is a configuration error.

The document's `lang` and `dir` follow selection. Use semantic/logical spacing for
new layouts, and review an RTL translation on desktop and mobile: setting `dir`
alone does not mirror illustrations or every physical positioning utility.

## Write translatable UI

Use the hook in each component so labels update without remounting:

```tsx
const { t, i18n } = useTranslation();
return <p>{t('Hello, {{name}}.', { name: user.name })}</p>;
```

Add the key and English value to `config/locales/en.json`, then to each enabled
catalog. Most keys are English source messages; periods and colons are literal
because key and namespace separators are disabled. Use complete sentences with
named interpolations instead of concatenating fragments. For rich text, use
`Trans` with `i18nKey` and explicit React components, as in the landing headline.
Never insert translated HTML with `dangerouslySetInnerHTML`. Interpolation is not
HTML-escaped by i18next because React escapes the final rendered text.

For plurals, use i18next JSON v4 suffixes and pass `count`:

```json
{
  "teamMembers_one": "{{count}} team member",
  "teamMembers_other": "{{count}} team members"
}
```

```tsx
t('teamMembers', { count: members.length });
t('{{used, number}} of {{limit, number}} seats in use', { used, limit });
new Date(expiresAt).toLocaleDateString(i18n.language);
```

Supply all cardinal or ordinal categories required by `Intl.PluralRules` for the
target locale, which may differ from English. Use the selected language with
`Intl.NumberFormat` / `Intl.DateTimeFormat` or i18next's Intl formatting. Language
selection does not change the user's time zone or billing currency.

Keep API identifiers, authorization roles, query keys, URLs and user-entered data
unchanged; translate their display labels. Shared `@appshell/ui` controls stay
independent of i18next and accept localized labels through props. App adapters
localize loading, errors, validation, retry and appearance controls.

## Optional translation lint

`checkMissingTranslations` defaults to `false`. Set it to `true` to make the
existing `npm run check:web`, `npm run check`, Git hooks and CI reject incomplete
translations. Or run `npm run check:i18n -- --strict` for a one-off check without
enabling the gate. Basic language configuration validation always runs.

The checker validates enabled catalogs against the English source catalog:

- Missing, empty, non-string and unknown entries.
- Interpolation variable mismatches.
- Locale-specific plural forms (including ordinal forms).
- Literal `t('…')`, `i18n.t('…')` and `Trans i18nKey="…"` references missing from English.

This is a completeness check, not a translation-quality check. It cannot prove
coverage of computed keys, rich-text semantics, or newly hardcoded UI strings.
Register dynamic display keys (for example role/status values or API messages)
explicitly in the source catalog and review new UI for untranslated text.
Disabling lint permits partial translations and runtime fallback.

## Backend boundary

This feature localizes the browser application. Existing API errors are translated
at display time using their message as a catalog key; unknown future messages keep
the server's original text until cataloged. The API contract, stored user content,
transactional emails, Stripe-hosted pages and the separate project website retain
their existing behavior. Locale-aware emails would require an explicit persisted
recipient preference and outbox/template work; browser storage is not available to
the mail worker.

References: [i18next configuration](https://www.i18next.com/overview/configuration-options),
[plural conventions](https://www.i18next.com/translation-function/plurals),
[React translation hooks](https://react.i18next.com/latest/usetranslation-hook),
and [BCP 47 lookup](https://www.rfc-editor.org/rfc/rfc4647#section-3.4).
