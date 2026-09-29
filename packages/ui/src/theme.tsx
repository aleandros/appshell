import { useEffect, useState } from 'react';
import { Monitor, Moon, Sun } from 'lucide-react';
import { Button } from './components';
type Theme = 'light' | 'dark' | 'system';
function savedTheme(): Theme {
  try {
    const value = localStorage.getItem('appshell-theme');
    return value === 'light' || value === 'dark' ? value : 'system';
  } catch {
    return 'system';
  }
}
export function ThemeSwitcher() {
  const [theme, setTheme] = useState<Theme>(savedTheme);
  useEffect(() => {
    const synchronize = () => setTheme(savedTheme());
    window.addEventListener('storage', synchronize);
    window.addEventListener('appshell-theme-change', synchronize);
    return () => {
      window.removeEventListener('storage', synchronize);
      window.removeEventListener('appshell-theme-change', synchronize);
    };
  }, []);
  useEffect(() => {
    const media = matchMedia('(prefers-color-scheme: dark)');
    const apply = () => {
      document.documentElement.dataset.theme =
        theme === 'system' ? (media.matches ? 'dark' : 'light') : theme;
    };
    apply();
    try {
      localStorage.setItem('appshell-theme', theme);
    } catch {
      /* Private browsing can disable storage. */
    }
    media.addEventListener('change', apply);
    return () => media.removeEventListener('change', apply);
  }, [theme]);
  return (
    <div
      className="inline-flex gap-1 rounded-xl border bg-surface p-1"
      role="group"
      aria-label="Color theme"
    >
      {(
        [
          { value: 'light', icon: Sun },
          { value: 'dark', icon: Moon },
          { value: 'system', icon: Monitor },
        ] as const
      ).map(({ value, icon: Icon }) => (
        <Button
          variant="quiet"
          key={value}
          title={`${value} theme`}
          aria-label={`${value} theme`}
          aria-pressed={theme === value}
          onClick={() => {
            setTheme(value);
            try {
              localStorage.setItem('appshell-theme', value);
              window.dispatchEvent(new Event('appshell-theme-change'));
            } catch {
              /* The selected theme still works without persistent storage. */
            }
          }}
          className={`flex h-9 min-h-9 w-9 px-0 py-0 items-center justify-center rounded-lg ${theme === value ? 'bg-accent text-accent-foreground' : 'text-muted hover:bg-surface-muted'}`}
        >
          <Icon size={16} />
        </Button>
      ))}
    </div>
  );
}
