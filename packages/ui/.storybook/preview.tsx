import { useEffect } from 'react';
import type { ReactNode } from 'react';
import type { Preview } from '@storybook/react-vite';
import '../src/styles.css';
function Theme({ theme, children }: { theme: string; children: ReactNode }) {
  useEffect(() => {
    const media = matchMedia('(prefers-color-scheme: dark)');
    const apply = () => {
      localStorage.setItem('appshell-theme', theme);
      window.dispatchEvent(new Event('appshell-theme-change'));
      document.documentElement.dataset.theme =
        theme === 'system' ? (media.matches ? 'dark' : 'light') : theme;
    };
    apply();
    media.addEventListener('change', apply);
    return () => media.removeEventListener('change', apply);
  }, [theme]);
  return <div className="min-h-screen bg-background p-6 text-foreground">{children}</div>;
}
const preview: Preview = {
  globalTypes: {
    theme: {
      description: 'Color theme',
      toolbar: { icon: 'paintbrush', items: ['light', 'dark', 'system'], dynamicTitle: true },
    },
  },
  initialGlobals: { theme: 'light' },
  parameters: { layout: 'fullscreen', a11y: { test: 'error' } },
  decorators: [
    (Story, context) => (
      <Theme theme={String(context.globals.theme)}>
        <Story />
      </Theme>
    ),
  ],
};
export default preview;
