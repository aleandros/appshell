import { ButtonLink } from '../components/links';
import { Link, useNavigate } from '@tanstack/react-router';
import { useState } from 'react';
import { z } from 'zod';
import { ArrowLeft, Check, Sprout } from 'lucide-react';
import { ActionForm, Field, Logo, Notice, Plant } from '../components/ui';
import { ThemeSwitcher } from '../components/theme';
import { queryClient } from '../lib/query-client';
import { identityApi } from '../features/identity';
import { organizationsApi } from '../features/organizations';
import { email, loginSchema, password, signupSchema } from '../lib/schemas';
import type { ReactNode } from 'react';

export function AuthLayout({
  title,
  description,
  children,
}: {
  title: string;
  description: string;
  children: ReactNode;
}) {
  return (
    <main className="grid min-h-dvh lg:grid-cols-2">
      <aside className="relative hidden flex-col justify-between overflow-hidden border-r bg-accent/35 p-12 lg:flex">
        <Link to="/">
          <Logo />
        </Link>
        <div className="max-w-md">
          <p className="eyebrow mb-5">A LITTLE ROOM TO GROW</p>
          <h2 className="text-5xl leading-tight font-semibold tracking-tight">
            Your next chapter
            <br />
            starts here.
          </h2>
          <p className="mt-6 max-w-sm text-base leading-7 text-muted">
            A thoughtful home for your work and the people who make it happen.
          </p>
          <div className="mt-10 flex gap-6 text-xs text-muted">
            <span className="flex gap-2">
              <Check size={15} /> Simple by design
            </span>
            <span className="flex gap-2">
              <Check size={15} /> Yours to make
            </span>
          </div>
        </div>
        <div className="flex items-end justify-between">
          <p className="text-xs text-muted">Space to build. Room to become.</p>
          <Plant />
        </div>
      </aside>
      <section className="flex flex-col p-6 sm:p-10">
        <header className="flex items-center justify-between">
          <ButtonLink variant="quiet" to="/" className="-ml-3 text-xs">
            <ArrowLeft size={15} /> Back to home
          </ButtonLink>
          <ThemeSwitcher />
        </header>
        <div className="mx-auto my-auto w-full max-w-sm py-12">
          <span className="mb-7 flex h-12 w-12 items-center justify-center rounded-2xl bg-accent text-primary">
            <Sprout size={25} />
          </span>
          <h1 className="text-3xl font-semibold tracking-tight">{title}</h1>
          <p className="mt-3 mb-8 leading-6 text-muted">{description}</p>
          {children}
        </div>
        <footer className="text-center text-xs text-muted">A good place to begin.</footer>
      </section>
    </main>
  );
}
export function SignupPage() {
  const navigate = useNavigate();
  return (
    <AuthLayout
      title="Make a little space."
      description="Create an account and a workspace for your next big thing."
    >
      <ActionForm
        schema={signupSchema}
        label="Create your workspace"
        submit={async (body) => {
          const result = await identityApi.signup(body);
          queryClient.clear();
          queryClient.setQueryData(['session'], result);
        }}
        onSuccess={() => navigate({ to: '/app' })}
      >
        <Field
          label="Your name"
          name="name"
          autoComplete="name"
          placeholder="Alex Morgan"
          required
          maxLength={80}
        />
        <Field
          label="Workspace name"
          name="organization"
          autoComplete="organization"
          placeholder="Acme Studio"
          required
          maxLength={80}
        />
        <Field
          label="Email address"
          name="email"
          type="email"
          autoComplete="email"
          placeholder="you@company.com"
          required
        />
        <Field
          label="Password"
          name="password"
          type="password"
          autoComplete="new-password"
          hint="At least 12 characters. Make it memorable."
          required
          minLength={12}
        />
      </ActionForm>
      <p className="mt-7 text-center text-sm text-muted">
        Already have an account?{' '}
        <Link to="/login" className="font-semibold text-primary">
          Sign in
        </Link>
      </p>
    </AuthLayout>
  );
}
export function LoginPage() {
  const navigate = useNavigate();
  return (
    <AuthLayout
      title="Welcome back."
      description="Your people and your next big idea are right where you left them."
    >
      <ActionForm
        schema={loginSchema}
        label="Sign in"
        submit={async (body) => {
          const result = await identityApi.login(body);
          queryClient.clear();
          queryClient.setQueryData(['session'], result);
        }}
        onSuccess={() => {
          const pending = sessionStorage.getItem('appshell-invite');
          return navigate({ to: pending ? '/accept-invite' : '/app' });
        }}
      >
        <Field
          label="Email address"
          name="email"
          type="email"
          autoComplete="email"
          placeholder="you@company.com"
          required
        />
        <Field
          label="Password"
          name="password"
          type="password"
          autoComplete="current-password"
          required
        />
        <div className="text-right">
          <Link to="/forgot-password" className="text-xs font-medium text-primary">
            Forgot your password?
          </Link>
        </div>
      </ActionForm>
      <p className="mt-7 text-center text-sm text-muted">
        New here?{' '}
        <Link to="/signup" className="font-semibold text-primary">
          Create an account
        </Link>
      </p>
    </AuthLayout>
  );
}
export function ForgotPage() {
  return (
    <AuthLayout
      title="Let’s get you back in."
      description="Enter your email and we’ll send you a password reset link."
    >
      <ActionForm
        schema={z.object({ email })}
        label="Send reset link"
        submit={(body) => identityApi.forgotPassword(body)}
        successMessage="If an account exists, a reset link is on its way. Check your inbox."
      >
        <Field label="Email address" name="email" type="email" autoComplete="email" required />
      </ActionForm>
      <ButtonLink variant="quiet" to="/login" className="mt-5 w-full">
        Back to sign in
      </ButtonLink>
    </AuthLayout>
  );
}
type TokenFlow = 'verify-email' | 'reset-password' | 'confirm-email' | 'accept-invite';
const tokenCopy: Record<
  TokenFlow,
  { title: string; description: string; label: string; success: string }
> = {
  'verify-email': {
    title: 'One small step.',
    description: 'Confirm your email to unlock your workspace.',
    label: 'Verify email',
    success: 'Email verified. Your workspace is ready.',
  },
  'reset-password': {
    title: 'A fresh start.',
    description: 'Choose a new password for your account.',
    label: 'Reset password',
    success: 'Password updated. Sign in with your new password.',
  },
  'confirm-email': {
    title: 'Confirm your new email.',
    description: 'Your account will use this address once you confirm.',
    label: 'Confirm email',
    success: 'Email updated. Sign in with your new address.',
  },
  'accept-invite': {
    title: 'You’re invited.',
    description:
      'Sign in with the email that received this invitation, verify it, then join your team.',
    label: 'Join workspace',
    success: 'You’re in. Find your new team in the workspace switcher.',
  },
};
export function TokenPage({ flow }: { flow: TokenFlow }) {
  const [token] = useState(() => {
    const value = new URLSearchParams(window.location.hash.slice(1)).get('token');
    if (flow === 'accept-invite' && value) sessionStorage.setItem('appshell-invite', value);
    return value ?? (flow === 'accept-invite' ? sessionStorage.getItem('appshell-invite') : null);
  });
  const [done, setDone] = useState(false);
  const copy = tokenCopy[flow];
  return (
    <AuthLayout title={copy.title} description={copy.description}>
      {!token ? (
        <Notice kind="error">
          This link is missing its token. Open the full link from your email.
        </Notice>
      ) : done ? (
        <Notice kind="success">{copy.success}</Notice>
      ) : (
        <ActionForm
          schema={flow === 'reset-password' ? z.object({ password }) : z.object({})}
          label={copy.label}
          submit={(body) =>
            flow === 'accept-invite'
              ? organizationsApi.accept(token)
              : identityApi.completeAction(flow, { ...body, token })
          }
          onSuccess={async () => {
            setDone(true);
            if (flow === 'accept-invite') sessionStorage.removeItem('appshell-invite');
            if (flow === 'reset-password' || flow === 'confirm-email') queryClient.clear();
            else await queryClient.invalidateQueries({ queryKey: ['session'] });
            window.history.replaceState(null, '', window.location.pathname);
          }}
        >
          {flow === 'reset-password' && (
            <Field
              label="New password"
              name="password"
              type="password"
              autoComplete="new-password"
              minLength={12}
              required
              hint="At least 12 characters."
            />
          )}
        </ActionForm>
      )}
      <div className="mt-6 flex flex-wrap justify-center gap-3">
        <ButtonLink variant="secondary" to="/login">
          Sign in
        </ButtonLink>
        {flow === 'accept-invite' && (
          <ButtonLink variant="secondary" to="/signup">
            Create an account
          </ButtonLink>
        )}
        <ButtonLink variant="quiet" to="/app">
          Go to workspace
        </ButtonLink>
      </div>
    </AuthLayout>
  );
}
