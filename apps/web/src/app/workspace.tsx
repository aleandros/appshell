import { LanguageSwitcher } from '../components/language-switcher';
import { useTranslation } from 'react-i18next';
import { Select } from '@appshell/ui';
import { NavigationLink } from '../components/links';
import { useState } from 'react';
import { WorkspaceContext } from '../lib/workspace-context';
import { Link, Outlet, useNavigate } from '@tanstack/react-router';
import { useMutation, useSuspenseQuery } from '@tanstack/react-query';
import {
  ArrowUpRight,
  ChevronDown,
  CreditCard,
  LayoutDashboard,
  LogOut,
  Menu,
  Plus,
  Settings2,
  Users,
  X,
} from 'lucide-react';
import { errorMessage } from '../lib/errors';
import { queryClient } from '../lib/query-client';
import { identityApi, sessionQuery } from '../features/identity';
import { Badge, Button, Logo, Notice } from '../components/ui';
import { ThemeSwitcher } from '../components/theme';

export function WorkspaceLayout() {
  const { t } = useTranslation();
  const links = [
    { to: '/app', label: t('Overview'), icon: LayoutDashboard },
    { to: '/app/team', label: t('Team members'), icon: Users },
    { to: '/app/billing', label: t('Plans & billing'), icon: CreditCard },
    { to: '/app/settings', label: t('Settings'), icon: Settings2 },
  ] as const;

  const { data: session } = useSuspenseQuery(sessionQuery);
  const navigate = useNavigate();
  const [chosen, setChosen] = useState<string | null>(null);
  const [open, setOpen] = useState(false);
  const organization =
    session.organizations.find((org) => org.id === chosen) ?? session.organizations[0];
  const logout = useMutation({
    mutationFn: identityApi.logout,
    onSuccess: async () => {
      queryClient.clear();
      await navigate({ to: '/login' });
    },
  });
  if (!organization)
    return (
      <div className="mx-auto max-w-lg p-8">
        <Notice>
          {t('Your account has no workspace. Sign out and contact the workspace owner.')}
        </Notice>
        <Button onClick={() => logout.mutate()} pending={logout.isPending}>
          {t('Sign out')}
        </Button>
      </div>
    );
  const sidebar = (
    <>
      <div className="mb-9 flex items-center justify-between">
        <Link to="/" aria-label={t('AppShell home')}>
          <Logo />
        </Link>
        <Button
          variant="quiet"
          onClick={() => setOpen(false)}
          aria-label={t('Close navigation')}
          className="lg:hidden"
        >
          <X size={20} />
        </Button>
      </div>
      <div className="relative mb-8">
        <label className="sr-only" htmlFor="workspace">
          {t('Current workspace')}
        </label>
        <Select
          id="workspace"
          className="appearance-none py-3.5 pr-9 font-semibold"
          value={organization.id}
          onChange={(event) => {
            setChosen(event.target.value);
            setOpen(false);
          }}
        >
          {session.organizations.map((org) => (
            <option key={org.id} value={org.id}>
              {org.name}
            </option>
          ))}
        </Select>
        <ChevronDown size={15} className="pointer-events-none absolute top-4 right-3 text-muted" />
      </div>
      <p className="eyebrow mb-3 px-3.5">{t('WORKSPACE')}</p>
      <nav className="space-y-1" aria-label={t('Workspace navigation')}>
        {links.map(({ to, label, icon: Icon }) => (
          <NavigationLink
            key={to}
            to={to}
            activeOptions={{ exact: true }}

            onClick={() => setOpen(false)}
          >
            <Icon size={19} />
            {label}
          </NavigationLink>
        ))}
      </nav>
      <NavigationLink to="/app/new-workspace" className="mt-4" onClick={() => setOpen(false)}>
        <Plus size={18} /> {t('New workspace')}
      </NavigationLink>
      <div className="mt-auto pt-10">
        <div className="rounded-2xl border bg-background p-4">
          <span className="text-lg">✦</span>
          <p className="mt-3 text-sm font-semibold">{t('A little room to grow.')}</p>
          <p className="mt-2 text-xs leading-5 text-muted">
            {t('More people. More possibilities.')}
            <br />
            {t('Find the plan that fits your team.')}
          </p>
          <Link
            to="/app/billing"
            className="mt-4 flex items-center justify-between text-xs font-semibold text-primary"
            onClick={() => setOpen(false)}
          >
            {t('Explore plans')}
            <ArrowUpRight size={15} />
          </Link>
        </div>
        <div className="mt-6 flex items-center gap-3 border-t pt-5">
          <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-accent font-semibold text-accent-foreground">
            {session.user.name.slice(0, 1).toUpperCase()}
          </span>
          <div className="min-w-0 flex-1">
            <p className="truncate text-xs font-semibold">{session.user.name}</p>
            <p className="mt-1 truncate text-[11px] text-muted">{session.user.email}</p>
          </div>
          <Button
            variant="quiet"
            aria-label={t('Sign out')}
            onClick={() => logout.mutate()}
            disabled={logout.isPending}
            className="px-2"
          >
            <LogOut size={17} />
          </Button>
        </div>
        {logout.isError && <Notice kind="error">{errorMessage(logout.error, t)}</Notice>}
      </div>
    </>
  );
  return (
    <WorkspaceContext.Provider value={{ session, organization }}>
      <a href="#main-content" className="sr-only z-50 bg-surface p-3 focus:not-sr-only focus:fixed">
        {t('Skip to content')}
      </a>
      <div className="min-h-dvh lg:pl-64">
        <aside className="fixed inset-y-0 left-0 hidden w-64 flex-col overflow-y-auto overscroll-contain border-r bg-surface p-6 lg:flex">
          {sidebar}
        </aside>
        {open && (
          <div className="fixed inset-x-0 top-0 z-40 max-h-dvh overflow-y-auto border-b bg-surface p-6 shadow-xl lg:hidden">
            {sidebar}
          </div>
        )}
        <header className="flex min-h-20 items-center justify-between flex-wrap gap-4 border-b bg-surface/65 py-3 px-5 sm:px-10">
          <div className="flex items-center gap-3">
            <Button
              variant="quiet"
              className="-ml-3 lg:hidden"
              aria-label={t('Open navigation')}
              aria-expanded={open}
              onClick={() => setOpen(true)}
            >
              <Menu size={21} />
            </Button>
            <span className="hidden text-xs text-muted sm:inline">{t('Workspace')}</span>
            <span className="hidden text-border sm:inline">/</span>
            <span className="max-w-32 truncate text-xs font-medium sm:max-w-64">
              {organization.name}
            </span>
            <Badge>{t(organization.role)}</Badge>
          </div>
          <div className="flex flex-wrap items-center justify-end gap-2">
            <LanguageSwitcher />
            <ThemeSwitcher />
          </div>
        </header>
        <main id="main-content" className="mx-auto max-w-7xl p-5 py-8 sm:p-10 lg:p-12">
          <Outlet />
        </main>
        <footer className="mx-auto flex max-w-7xl flex-wrap justify-between gap-2 px-5 pt-5 pb-8 text-[11px] text-muted sm:px-10 lg:px-12">
          <span>{t('AppShell · Space to build.')}</span>
          <span>{t('Thoughtfully simple. Endlessly yours.')}</span>
        </footer>
      </div>
    </WorkspaceContext.Provider>
  );
}
