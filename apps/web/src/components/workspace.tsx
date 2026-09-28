import { createContext, useContext, useState } from 'react';
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
import { api, errorMessage, queryClient, sessionQuery } from '../lib/api';
import { messageSchema } from '../lib/schemas';
import type { Organization, Session } from '../lib/schemas';
import { Badge, Button, Logo, Notice } from './ui';
import { ThemeSwitcher } from './theme';

const WorkspaceContext = createContext<{ session: Session; organization: Organization } | null>(
  null,
);
export function useWorkspace() {
  const value = useContext(WorkspaceContext);
  if (!value) throw new Error('Workspace context is missing');
  return value;
}
const links = [
  { to: '/app', label: 'Overview', icon: LayoutDashboard },
  { to: '/app/team', label: 'Team members', icon: Users },
  { to: '/app/billing', label: 'Plans & billing', icon: CreditCard },
  { to: '/app/settings', label: 'Settings', icon: Settings2 },
] as const;
export function WorkspaceLayout() {
  const { data: session } = useSuspenseQuery(sessionQuery);
  const navigate = useNavigate();
  const [chosen, setChosen] = useState<string | null>(null);
  const [open, setOpen] = useState(false);
  const organization =
    session.organizations.find((org) => org.id === chosen) ?? session.organizations[0];
  const logout = useMutation({
    mutationFn: () => api('/auth/logout', messageSchema, { method: 'POST' }),
    onSuccess: async () => {
      queryClient.clear();
      await navigate({ to: '/login' });
    },
  });
  if (!organization)
    return (
      <div className="mx-auto max-w-lg p-8">
        <Notice>Your account has no workspace. Sign out and contact the workspace owner.</Notice>
        <Button onClick={() => logout.mutate()} pending={logout.isPending}>
          Sign out
        </Button>
      </div>
    );
  const sidebar = (
    <>
      <div className="mb-9 flex items-center justify-between">
        <Link to="/" aria-label="AppShell home">
          <Logo />
        </Link>
        <button
          onClick={() => setOpen(false)}
          aria-label="Close navigation"
          className="btn btn-quiet lg:hidden"
        >
          <X size={20} />
        </button>
      </div>
      <div className="relative mb-8">
        <label className="sr-only" htmlFor="workspace">
          Current workspace
        </label>
        <select
          id="workspace"
          className="input appearance-none py-3.5 pr-9 font-semibold"
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
        </select>
        <ChevronDown size={15} className="pointer-events-none absolute top-4 right-3 text-muted" />
      </div>
      <p className="eyebrow mb-3 px-3.5">WORKSPACE</p>
      <nav className="space-y-1" aria-label="Workspace navigation">
        {links.map(({ to, label, icon: Icon }) => (
          <Link
            key={to}
            to={to}
            activeOptions={{ exact: true }}
            className="nav-item"
            onClick={() => setOpen(false)}
          >
            <Icon size={19} />
            {label}
          </Link>
        ))}
      </nav>
      <Link to="/app/new-workspace" className="nav-item mt-4" onClick={() => setOpen(false)}>
        <Plus size={18} /> New workspace
      </Link>
      <div className="mt-auto pt-10">
        <div className="rounded-2xl border bg-background p-4">
          <span className="text-lg">✦</span>
          <p className="mt-3 text-sm font-semibold">A little room to grow.</p>
          <p className="mt-2 text-xs leading-5 text-muted">
            More people. More possibilities.
            <br />
            Find the plan that fits your team.
          </p>
          <Link
            to="/app/billing"
            className="mt-4 flex items-center justify-between text-xs font-semibold text-primary"
            onClick={() => setOpen(false)}
          >
            Explore plans <ArrowUpRight size={15} />
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
          <button
            aria-label="Sign out"
            onClick={() => logout.mutate()}
            disabled={logout.isPending}
            className="btn btn-quiet px-2"
          >
            <LogOut size={17} />
          </button>
        </div>
        {logout.isError && <Notice kind="error">{errorMessage(logout.error)}</Notice>}
      </div>
    </>
  );
  return (
    <WorkspaceContext.Provider value={{ session, organization }}>
      <a href="#main-content" className="sr-only z-50 bg-surface p-3 focus:not-sr-only focus:fixed">
        Skip to content
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
        <header className="flex min-h-20 items-center justify-between gap-4 border-b bg-surface/65 px-5 sm:px-10">
          <div className="flex items-center gap-3">
            <button
              className="btn btn-quiet -ml-3 lg:hidden"
              aria-label="Open navigation"
              aria-expanded={open}
              onClick={() => setOpen(true)}
            >
              <Menu size={21} />
            </button>
            <span className="hidden text-xs text-muted sm:inline">Workspace</span>
            <span className="hidden text-border sm:inline">/</span>
            <span className="max-w-32 truncate text-xs font-medium sm:max-w-64">
              {organization.name}
            </span>
            <Badge>{organization.role}</Badge>
          </div>
          <ThemeSwitcher />
        </header>
        <main id="main-content" className="mx-auto max-w-7xl p-5 py-8 sm:p-10 lg:p-12">
          <Outlet />
        </main>
        <footer className="mx-auto flex max-w-7xl flex-wrap justify-between gap-2 px-5 pt-5 pb-8 text-[11px] text-muted sm:px-10 lg:px-12">
          <span>AppShell · Space to build.</span>
          <span>Thoughtfully simple. Endlessly yours.</span>
        </footer>
      </div>
    </WorkspaceContext.Provider>
  );
}
