import { ThemeSwitcher as SharedThemeSwitcher } from '@appshell/ui';
import { useTranslation } from 'react-i18next';

export function ThemeSwitcher() {
  const { t } = useTranslation();
  return (
    <SharedThemeSwitcher
      labels={{
        group: t('Color theme'),
        light: t('light theme'),
        dark: t('dark theme'),
        system: t('system theme'),
      }}
    />
  );
}
