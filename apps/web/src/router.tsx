import { useTranslation } from 'react-i18next';
import { LanguageSwitcher } from './components/language-switcher';
import { AdminLoginPage, AdminPage } from './pages/admin';
import { adminSessionQuery } from './features/admin';
import { ButtonLink } from './components/links';
import {
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  redirect,
} from '@tanstack/react-router';
import { ApiError } from './lib/errors';
import { queryClient } from './lib/query-client';
import { sessionQuery } from './features/identity';
import { ErrorState, Loading } from './components/ui';
import { Landing } from './pages/landing';
import { ForgotPage, LoginPage, SignupPage, TokenPage } from './pages/auth';
import { WorkspaceLayout } from './app/workspace';
import {
  BillingPage,
  NewWorkspacePage,
  OverviewPage,
  SettingsPage,
  TeamPage,
} from './pages/workspace';
function RouteError({ error, reset }: { error: unknown; reset: () => void }) {
  const { t } = useTranslation();
  return (
    <div className="p-8">
      <LanguageSwitcher />
      <ErrorState error={error} retry={reset} />
      <ButtonLink variant="secondary" to="/">
        {t('Back to home')}
      </ButtonLink>
    </div>
  );
}
function NotFound() {
  const { t } = useTranslation();
  return (
    <div className="mx-auto max-w-lg space-y-5 p-10">
      <LanguageSwitcher />
      <p className="eyebrow">{t('404 · A LITTLE OFF TRACK')}</p>
      <h1 className="page-title">{t('This space isn’t here.')}</h1>
      <p className="text-muted">{t('The page may have moved, or the link might be incomplete.')}</p>
      <ButtonLink variant="primary" to="/">
        {t('Back to home')}
      </ButtonLink>
    </div>
  );
}
const root = createRootRoute({
  component: Outlet,
  errorComponent: RouteError,
  notFoundComponent: NotFound,
});
const landing = createRoute({ getParentRoute: () => root, path: '/', component: Landing });
const login = createRoute({ getParentRoute: () => root, path: '/login', component: LoginPage });
const signup = createRoute({ getParentRoute: () => root, path: '/signup', component: SignupPage });
const forgot = createRoute({
  getParentRoute: () => root,
  path: '/forgot-password',
  component: ForgotPage,
});
const verify = createRoute({
  getParentRoute: () => root,
  path: '/verify-email',
  component: () => <TokenPage flow="verify-email" />,
});
const reset = createRoute({
  getParentRoute: () => root,
  path: '/reset-password',
  component: () => <TokenPage flow="reset-password" />,
});
const confirm = createRoute({
  getParentRoute: () => root,
  path: '/confirm-email',
  component: () => <TokenPage flow="confirm-email" />,
});
const invite = createRoute({
  getParentRoute: () => root,
  path: '/accept-invite',
  component: () => <TokenPage flow="accept-invite" />,
});
const adminLogin = createRoute({
  getParentRoute: () => root,
  path: '/admin/login',
  component: AdminLoginPage,
});
const admin = createRoute({
  getParentRoute: () => root,
  path: '/admin',
  component: AdminPage,
  beforeLoad: async () => {
    try {
      await queryClient.query(adminSessionQuery);
    } catch (error) {
      if (error instanceof ApiError && error.status === 401)
        redirect({ to: '/admin/login', throw: true });
      throw error;
    }
  },
});
const workspace = createRoute({
  getParentRoute: () => root,
  path: '/app',
  component: WorkspaceLayout,
  pendingComponent: Loading,
  beforeLoad: async () => {
    try {
      await queryClient.query(sessionQuery);
    } catch (error) {
      if (error instanceof ApiError && error.status === 401) {
        redirect({ to: '/login', throw: true });
      }
      throw error;
    }
  },
});
const overview = createRoute({
  getParentRoute: () => workspace,
  path: '/',
  component: OverviewPage,
});
const team = createRoute({ getParentRoute: () => workspace, path: '/team', component: TeamPage });
const billing = createRoute({
  getParentRoute: () => workspace,
  path: '/billing',
  component: BillingPage,
});
const settings = createRoute({
  getParentRoute: () => workspace,
  path: '/settings',
  component: SettingsPage,
});
const newWorkspace = createRoute({
  getParentRoute: () => workspace,
  path: '/new-workspace',
  component: NewWorkspacePage,
});
const routeTree = root.addChildren([
  adminLogin,
  admin,
  landing,
  login,
  signup,
  forgot,
  verify,
  reset,
  confirm,
  invite,
  workspace.addChildren([overview, team, billing, settings, newWorkspace]),
]);
export const router = createRouter({
  routeTree,
  defaultPreload: 'intent',
  defaultPendingComponent: Loading,
  defaultPendingMs: 200,
  scrollRestoration: true,
});
declare module '@tanstack/react-router' {
  interface Register {
    router: typeof router;
  }
}
