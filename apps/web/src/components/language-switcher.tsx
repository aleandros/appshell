import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Notice, Select } from '@appshell/ui';
import { localeConfig, selectLanguage } from '../lib/i18n';

export function LanguageSwitcher() {
  const { t, i18n } = useTranslation();
  const id = useId();
  const [pending, setPending] = useState(false);
  const [failed, setFailed] = useState(false);
  if (localeConfig.languages.length < 2) return null;
  return (
    <div className="max-w-48 space-y-2">
      <label htmlFor={id} className="sr-only">
        {t('Language')}
      </label>
      <Select
        id={id}
        value={i18n.language}
        disabled={pending}
        aria-busy={pending}
        onChange={(event) => {
          setPending(true);
          setFailed(false);
          void selectLanguage(event.target.value)
            .catch(() => setFailed(true))
            .finally(() => setPending(false));
        }}
      >
        {localeConfig.languages.map(({ tag, label }) => (
          <option key={tag} value={tag} lang={tag}>
            {label}
          </option>
        ))}
      </Select>
      {failed && (
        <Notice kind="error">{t('Could not load this language. Please try again.')}</Notice>
      )}
    </div>
  );
}
