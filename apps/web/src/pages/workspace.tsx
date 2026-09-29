import { Card, Select } from '@appshell/ui';
import { ButtonLink, CardLink } from '../components/links';
import { Link, useNavigate } from '@tanstack/react-router';
import { useMutation, useQuery } from '@tanstack/react-query';
import {
  ArrowRight,
  ArrowUpRight,
  Check,
  CheckCircle2,
  Circle,
  CreditCard,
  Mail,
  Plus,
  ShieldCheck,
  Sparkles,
  Users,
} from 'lucide-react';
import { z } from 'zod';
import { useState } from 'react';
import {
  ActionForm,
  Badge,
  Button,
  ErrorState,
  Field,
  Loading,
  Notice,
  PageHeading,
  Plant,
} from '../components/ui';
import { useWorkspace } from '../lib/workspace-context';
import { errorMessage } from '../lib/errors';
import { queryClient } from '../lib/query-client';
import { identityApi } from '../features/identity';
import { organizationsApi, teamQuery } from '../features/organizations';
import { billingApi, billingQuery } from '../features/billing';
import { email, name, password } from '../lib/schemas';
import { ThemeSwitcher } from '../components/theme';

function useTeam() {
  const { organization, session } = useWorkspace();
  return useQuery({
    ...teamQuery(organization.id),
    enabled: !!session.user.email_verified_at,
  });
}
function useBilling() {
  const { organization, session } = useWorkspace();
  return useQuery({
    ...billingQuery(organization.id),
    enabled: !!session.user.email_verified_at,
  });
}
export function VerificationBanner() {
  const { session } = useWorkspace();
  const mutation = useMutation({
    mutationFn: identityApi.resendVerification,
  });
  if (session.user.email_verified_at) return null;
  return (
    <div className="mb-7">
      <Notice>
        <div className="flex flex-wrap items-center gap-3">
          <div>
            <strong>One small step: verify your email.</strong>
            <p>We sent a link to {session.user.email}. Open it to activate your workspace.</p>
          </div>
          <Button
            variant="secondary"
            pending={mutation.isPending}
            onClick={() => mutation.mutate()}
          >
            Resend link
          </Button>
        </div>
      </Notice>
      {mutation.isSuccess && (
        <p role="status" className="mt-2 text-sm text-primary">
          Verification email sent. Check your inbox.
        </p>
      )}
      {mutation.isError && <Notice kind="error">{errorMessage(mutation.error)}</Notice>}
    </div>
  );
}
export function OverviewPage() {
  const { session, organization } = useWorkspace();
  const team = useTeam();
  const billing = useBilling();
  const verified = !!session.user.email_verified_at;
  return (
    <>
      <PageHeading
        eyebrow="YOUR WORKSPACE, AT A GLANCE"
        title={`Hello, ${session.user.name.split(' ')[0] ?? 'there'}.`}
        description="A little structure for your next big thing."
        action={
          <ButtonLink variant="secondary" to="/app/team">
            <Plus size={16} /> Invite a teammate
          </ButtonLink>
        }
      />
      <VerificationBanner />
      <section className="relative mb-6 flex min-h-60 items-center justify-between gap-5 overflow-hidden rounded-2xl border bg-accent/45 p-7 sm:px-9">
        <div className="relative z-10 max-w-md">
          <Badge>
            <Sparkles size={12} /> A FRESH BEGINNING
          </Badge>
          <h2 className="mt-5 text-2xl font-semibold tracking-tight sm:text-3xl">
            Good things grow here.
          </h2>
          <p className="mt-3 max-w-sm leading-7 text-muted">
            Welcome to {organization.name}. Make yourself at home, bring your people, and turn a
            little possibility into something great.
          </p>
          <Link
            to="/app/settings"
            className="mt-5 inline-flex items-center gap-2 text-xs font-semibold text-primary"
          >
            Make it yours <ArrowRight size={15} />
          </Link>
        </div>
        <div className="absolute -right-12 bottom-5 opacity-25 sm:relative sm:right-auto sm:bottom-auto sm:mr-3 sm:shrink-0 sm:opacity-100">
          <Plant />
        </div>
      </section>
      <div className="mb-8 grid gap-4 sm:grid-cols-3">
        {[
          {
            icon: Users,
            label: 'Your people',
            value: team.data ? String(team.data.members.length) : verified ? '—' : '1',
            note: 'A good team starts with you',
            to: '/app/team',
          },
          {
            icon: CreditCard,
            label: 'Current plan',
            value: billing.data?.subscription.plan ?? 'Free',
            note: billing.data
              ? `${billing.data.seats_used} of ${billing.data.seat_limit} seats in use`
              : 'Room to grow at your pace',
            to: '/app/billing',
          },
          {
            icon: ShieldCheck,
            label: 'Account status',
            value: verified ? 'Verified' : 'One more step',
            note: verified ? 'Your email is confirmed' : 'Check your inbox to get started',
            to: '/app/settings',
          },
        ].map(({ icon: Icon, label, value, note, to }) => (
          <CardLink
            key={label}
            to={to}
            className="group p-6 transition-colors hover:border-primary/40"
          >
            <div className="flex items-center justify-between text-muted">
              <span className="text-xs">{label}</span>
              <Icon size={18} />
            </div>
            <p className="mt-5 text-2xl font-semibold capitalize tracking-tight">{value}</p>
            <div className="mt-2 flex items-center justify-between text-xs text-muted">
              <span>{note}</span>
              <ArrowUpRight
                size={14}
                className="transition-transform group-hover:translate-x-0.5"
              />
            </div>
          </CardLink>
        ))}
      </div>
      {(team.isError || billing.isError) && (
        <ErrorState
          error={team.error ?? billing.error}
          retry={() => {
            void team.refetch();
            void billing.refetch();
          }}
        />
      )}
      <div className="grid gap-6 xl:grid-cols-[1.35fr_1fr]">
        <Card className="p-6 sm:p-7">
          <div className="mb-6 flex items-center justify-between">
            <h2 className="text-lg font-semibold tracking-tight">A few small steps</h2>
            <Badge>
              {1 + Number(verified) + Number((team.data?.members.length ?? 1) > 1)} / 3 complete
            </Badge>
          </div>
          {[
            {
              title: 'Create your workspace',
              description: 'A space to call your own.',
              done: true,
              to: '/app',
            },
            {
              title: 'Verify your email',
              description: 'Keep your account in good hands.',
              done: verified,
              to: '/app/settings',
            },
            {
              title: 'Bring your first teammate',
              description: 'Good things are better together.',
              done: (team.data?.members.length ?? 1) > 1,
              to: '/app/team',
            },
          ].map(({ title, description, done, to }) => (
            <Link
              key={title}
              to={to}
              className="flex items-center gap-4 border-t py-5 first:border-0"
            >
              {done ? (
                <CheckCircle2 size={21} className="shrink-0 text-primary" />
              ) : (
                <Circle size={21} className="shrink-0 text-muted/40" />
              )}
              <div className="flex-1">
                <p className="text-sm font-medium">{title}</p>
                <p className="mt-1 text-xs text-muted">{description}</p>
              </div>
              <ArrowRight size={16} className="text-muted" />
            </Link>
          ))}
        </Card>
        <Card className="flex flex-col justify-between p-7">
          <div>
            <span className="mb-5 flex h-11 w-11 items-center justify-center rounded-xl bg-surface-muted text-primary">
              <Users size={22} />
            </span>
            <h2 className="text-xl font-semibold tracking-tight">Your people. Your space.</h2>
            <p className="mt-3 text-sm leading-7 text-muted">
              One workspace for the things you share. Invite a collaborator and start building
              something together.
            </p>
          </div>
          <ButtonLink variant="secondary" to="/app/team" className="mt-6 self-start">
            Meet your team <ArrowRight size={15} />
          </ButtonLink>
        </Card>
      </div>
    </>
  );
}
export function TeamPage() {
  const { organization, session } = useWorkspace();
  const team = useTeam();
  const [showInvite, setShowInvite] = useState(false);
  const canManage = organization.role !== 'member';
  const remove = useMutation({
    mutationFn: ({ id, kind }: { id: string; kind: 'members' | 'invitations' }) =>
      organizationsApi.remove(organization.id, kind, id),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ['team', organization.id] });
      await queryClient.invalidateQueries({ queryKey: ['billing', organization.id] });
    },
  });
  return (
    <>
      <PageHeading
        eyebrow="BETTER TOGETHER"
        title="Your people."
        description="A shared space for the people who make it happen."
        action={
          canManage && (
            <Button
              onClick={() => setShowInvite(!showInvite)}
              disabled={!session.user.email_verified_at}
            >
              <Plus size={16} /> {showInvite ? 'Close invitation form' : 'Invite a teammate'}
            </Button>
          )
        }
      />
      <VerificationBanner />
      {showInvite && (
        <Card className="mb-6 max-w-lg p-6">
          <h2 className="mb-5 text-lg font-semibold">A little invitation goes a long way.</h2>
          <ActionForm
            schema={z.object({ email, role: z.enum(['admin', 'member']) })}
            label="Send invitation"
            submit={(body) => organizationsApi.invite(organization.id, body)}
            successMessage="Invitation sent. Your teammate will receive a link by email."
            onSuccess={async () => {
              await queryClient.invalidateQueries({ queryKey: ['team', organization.id] });
            }}
          >
            <Field
              label="Email address"
              name="email"
              type="email"
              placeholder="teammate@company.com"
              required
            />
            <div>
              <label htmlFor="invite-role" className="label">
                Role
              </label>
              <Select id="invite-role" name="role" defaultValue="member">
                <option value="member">Member — collaborate in the workspace</option>
                {organization.role === 'owner' && (
                  <option value="admin">Admin — manage team invitations</option>
                )}
              </Select>
            </div>
          </ActionForm>
        </Card>
      )}
      {remove.isError && <Notice kind="error">{errorMessage(remove.error)}</Notice>}
      {remove.isSuccess && <Notice kind="success">{remove.data.message}</Notice>}
      {!session.user.email_verified_at ? null : team.isPending ? (
        <Loading label="Loading team members…" />
      ) : team.isError ? (
        <ErrorState error={team.error} retry={() => void team.refetch()} />
      ) : (
        <>
          <Card className="overflow-hidden">
            <div className="flex items-center justify-between border-b p-6">
              <h2 className="font-semibold">
                Workspace members{' '}
                <span className="ml-2 text-muted">{team.data.members.length}</span>
              </h2>
              <Badge>
                <span className="h-1.5 w-1.5 rounded-full bg-primary" /> Private workspace
              </Badge>
            </div>
            <div className="divide-y">
              {team.data.members.map((member) => (
                <div key={member.id} className="flex flex-wrap items-center gap-3 p-5 sm:px-6">
                  <span className="flex h-10 w-10 items-center justify-center rounded-full bg-surface-muted font-medium">
                    {member.name.slice(0, 1)}
                  </span>
                  <div className="min-w-0 flex-1">
                    <p className="truncate font-medium">
                      {member.name}
                      {member.id === session.user.id && (
                        <span className="ml-2 text-xs font-normal text-muted">(you)</span>
                      )}
                    </p>
                    <p className="mt-1 truncate text-xs text-muted">{member.email}</p>
                  </div>
                  <Badge>{member.role}</Badge>
                  {canManage &&
                    member.role !== 'owner' &&
                    (organization.role === 'owner' || member.role !== 'admin') &&
                    member.id !== session.user.id && (
                      <Button
                        variant="quiet"
                        pending={remove.isPending}
                        onClick={() => {
                          if (window.confirm(`Remove ${member.name} from this workspace?`))
                            remove.mutate({ id: member.id, kind: 'members' });
                        }}
                      >
                        Remove
                      </Button>
                    )}
                </div>
              ))}
            </div>
          </Card>
          <Card className="mt-6 p-6">
            <h2 className="font-semibold">Pending invitations</h2>
            {team.data.invitations.length === 0 ? (
              <div className="py-10 text-center">
                <Mail size={27} className="mx-auto mb-4 text-muted/60" />
                <h3 className="font-medium">No invitations in the air.</h3>
                <p className="mt-2 text-sm text-muted">
                  Your next great collaborator could be one invite away.
                </p>
              </div>
            ) : (
              team.data.invitations.map((invite) => (
                <div
                  key={invite.id}
                  className="mt-4 flex flex-wrap items-center justify-between gap-3 border-t pt-4"
                >
                  <div className="min-w-0">
                    <p className="break-all text-sm font-medium">{invite.email}</p>
                    <p className="mt-1 text-xs text-muted">
                      {invite.role} · Expires {new Date(invite.expires_at).toLocaleDateString()}
                    </p>
                  </div>
                  {canManage && (
                    <Button
                      variant="quiet"
                      pending={remove.isPending}
                      onClick={() => remove.mutate({ id: invite.id, kind: 'invitations' })}
                    >
                      Revoke
                    </Button>
                  )}
                </div>
              ))
            )}
          </Card>
        </>
      )}
    </>
  );
}
export function BillingPage() {
  const { organization, session } = useWorkspace();
  const billing = useBilling();
  const mutation = useMutation({
    mutationFn: (mode: 'checkout' | 'billing-portal') =>
      billingApi.openPortal(organization.id, mode),
    onSuccess: ({ url }) => {
      const parsed = new URL(url);
      if (
        parsed.protocol !== 'https:' ||
        !['checkout.stripe.com', 'billing.stripe.com'].includes(parsed.hostname)
      )
        throw new Error('Invalid billing destination.');
      window.location.assign(url);
    },
  });
  return (
    <>
      <PageHeading
        eyebrow="ROOM TO GROW"
        title="Your pace. Your plan."
        description="Start small. Make room when you’re ready."
      />
      <VerificationBanner />
      {!session.user.email_verified_at ? null : billing.isPending ? (
        <Loading />
      ) : billing.isError ? (
        <ErrorState error={billing.error} retry={() => void billing.refetch()} />
      ) : (
        <>
          <Card className="mb-6 flex flex-wrap items-center justify-between gap-5 p-7">
            <div>
              <p className="eyebrow">YOUR CURRENT PLAN</p>
              <div className="mt-3 flex items-center gap-3">
                <h2 className="text-2xl font-semibold capitalize">
                  {billing.data.subscription.plan}
                </h2>
                <Badge>{billing.data.subscription.status.replaceAll('_', ' ')}</Badge>
              </div>
              <p className="mt-3 text-sm text-muted">
                {billing.data.seats_used} of {billing.data.seat_limit} seats in use
                {billing.data.subscription.current_period_end &&
                  ` · Current period ends ${new Date(billing.data.subscription.current_period_end).toLocaleDateString()}`}
              </p>
            </div>
            <CreditCard size={36} strokeWidth={1.2} className="text-primary" />
          </Card>
          {!billing.data.billing_enabled && (
            <Notice>
              Paid plans aren’t enabled for this installation yet. Your free workspace is ready to
              use.
            </Notice>
          )}
          {mutation.isError && <Notice kind="error">{errorMessage(mutation.error)}</Notice>}
          <div className="mt-6 grid gap-6 md:grid-cols-2">
            {[
              {
                plan: 'Free',
                tag: 'FOR SMALL BEGINNINGS',
                description: 'A comfortable place to get started.',
                features: [
                  'Up to 3 workspace members',
                  'Private organization workspace',
                  'All account and security essentials',
                ],
              },
              {
                plan: 'Pro',
                tag: 'FOR GROWING TOGETHER',
                description: 'A little more room for your next chapter.',
                features: [
                  'Up to 50 workspace members',
                  'Everything in Free',
                  'Self-service subscription management',
                ],
              },
            ].map(({ plan, tag, description, features }) => (
              <Card key={plan} className={`p-7 ${plan === 'Pro' ? 'border-primary/50' : ''}`}>
                <p className="eyebrow">{tag}</p>
                <h2 className="mt-4 text-3xl font-semibold">{plan}</h2>
                <p className="mt-3 text-sm text-muted">{description}</p>
                <p className="mt-6 text-lg font-medium">
                  {plan === 'Free' ? '$0 / forever' : 'See price at checkout'}
                </p>
                <ul className="my-7 space-y-4">
                  {features.map((feature) => (
                    <li key={feature} className="flex gap-3 text-sm">
                      <Check size={17} className="shrink-0 text-primary" />
                      {feature}
                    </li>
                  ))}
                </ul>
                {plan === 'Free' ? (
                  <Badge>Included with every workspace</Badge>
                ) : (
                  <Button
                    className="w-full"
                    disabled={!billing.data.billing_enabled || organization.role !== 'owner'}
                    pending={mutation.isPending}
                    onClick={() =>
                      mutation.mutate(
                        billing.data.subscription.plan === 'pro' ? 'billing-portal' : 'checkout',
                      )
                    }
                  >
                    {billing.data.subscription.plan === 'pro'
                      ? 'Manage subscription'
                      : 'Explore Pro'}
                    <ArrowUpRight size={16} />
                  </Button>
                )}
              </Card>
            ))}
          </div>
          {organization.role !== 'owner' && (
            <p className="mt-5 text-sm text-muted">Only the workspace owner can change billing.</p>
          )}
        </>
      )}
    </>
  );
}
export function SettingsPage() {
  const navigate = useNavigate();
  const { session } = useWorkspace();
  return (
    <>
      <PageHeading
        eyebrow="MAKE YOURSELF AT HOME"
        title="Your settings."
        description="The little details that make this space yours."
      />
      <VerificationBanner />
      <div className="grid items-start gap-6 xl:grid-cols-2">
        <Card className="p-7">
          <h2 className="text-lg font-semibold">Your account</h2>
          <div className="mt-6 space-y-4">
            <div>
              <p className="eyebrow">NAME</p>
              <p className="mt-2">{session.user.name}</p>
            </div>
            <div>
              <p className="eyebrow">EMAIL</p>
              <p className="mt-2 break-all">{session.user.email}</p>
            </div>
          </div>
          <div className="mt-7 border-t pt-6">
            <h3 className="font-medium">Appearance</h3>
            <p className="mt-2 mb-4 text-sm text-muted">A look that feels right, day or night.</p>
            <ThemeSwitcher />
          </div>
        </Card>
        <Card className="p-7">
          <h2 className="mb-2 text-lg font-semibold">Change your password</h2>
          <p className="mb-6 text-sm leading-6 text-muted">
            Updating your password signs you out on every device.
          </p>
          <ActionForm
            schema={z.object({ current_password: z.string().min(1), password })}
            label="Update password"
            submit={(body) => identityApi.changePassword(body)}
            onSuccess={async () => {
              queryClient.clear();
              await navigate({ to: '/login' });
            }}
          >
            <Field
              label="Current password"
              name="current_password"
              type="password"
              autoComplete="current-password"
              required
            />
            <Field
              label="New password"
              name="password"
              type="password"
              autoComplete="new-password"
              minLength={12}
              hint="At least 12 characters."
              required
            />
          </ActionForm>
        </Card>
        <Card className="p-7">
          <h2 className="mb-2 text-lg font-semibold">Change your email</h2>
          <p className="mb-6 text-sm leading-6 text-muted">
            We’ll send a confirmation link to your new address. Your current email stays active
            until you confirm.
          </p>
          <ActionForm
            schema={z.object({ email, password: z.string().min(1) })}
            label="Send confirmation"
            submit={(body) => identityApi.changeEmail(body)}
            successMessage="Check your new inbox for a confirmation link."
          >
            <Field
              label="New email address"
              name="email"
              type="email"
              autoComplete="email"
              required
            />
            <Field
              label="Current password"
              name="password"
              type="password"
              autoComplete="current-password"
              required
            />
          </ActionForm>
        </Card>
      </div>
    </>
  );
}
export function NewWorkspacePage() {
  const navigate = useNavigate();
  return (
    <>
      <PageHeading
        eyebrow="A NEW CHAPTER"
        title="Create a workspace."
        description="Give your next idea a space of its own."
      />
      <VerificationBanner />
      <Card className="max-w-lg p-7">
        <ActionForm
          schema={z.object({ name })}
          label="Create workspace"
          submit={(body) => organizationsApi.create(body)}
          onSuccess={async () => {
            await queryClient.invalidateQueries({ queryKey: ['session'] });
            await navigate({ to: '/app' });
          }}
        >
          <Field
            label="Workspace name"
            name="name"
            placeholder="Your next big thing"
            maxLength={80}
            required
          />
        </ActionForm>
      </Card>
    </>
  );
}
