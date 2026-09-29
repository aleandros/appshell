import { useState } from 'react';
import { useQuery, useMutation } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { z } from 'zod';
import {
  ActionForm,
  Button,
  Card,
  ErrorState,
  Field,
  Loading,
  Notice,
  PageHeading,
  Select,
  Badge,
} from '../components/ui';
import { ButtonLink } from '../components/links';
import { ThemeSwitcher } from '../components/theme';
import { queryClient } from '../lib/query-client';
import { loginSchema } from '../lib/schemas';
import {
  adminApi,
  adminSessionQuery,
  adminUsersQuery,
  adminAccountsQuery,
  adminHistoryQuery,
  createAdminSchema,
  updateUserSchema,
  updateAdminSchema,
} from '../features/admin';
import type { AdminAccount, ManagedUser } from '../features/admin';

export function AdminLoginPage() {
  const navigate = useNavigate();
  return (
    <main className="mx-auto min-h-dvh max-w-md space-y-7 px-6 py-12">
      <header className="flex items-center justify-between">
        <ButtonLink to="/" variant="quiet">
          Home
        </ButtonLink>
        <ThemeSwitcher />
      </header>
      <PageHeading
        eyebrow="ADMINISTRATION"
        title="Admin sign in"
        description="Manage accounts and review changes across this installation."
      />
      <Card className="p-6">
        <ActionForm
          schema={loginSchema}
          label="Sign in to administration"
          submit={adminApi.login}
          onSuccess={async () => {
            queryClient.removeQueries({ queryKey: ['admin'] });
            await navigate({ to: '/admin' });
          }}
        >
          <Field label="Admin email" name="email" type="email" autoComplete="username" required />
          <Field
            label="Admin password"
            name="password"
            type="password"
            autoComplete="current-password"
            required
          />
        </ActionForm>
      </Card>
    </main>
  );
}
function Pager({
  offset,
  count,
  setOffset,
}: {
  offset: number;
  count: number;
  setOffset: (offset: number) => void;
}) {
  return (
    <div className="flex flex-wrap items-center gap-3 pt-4">
      <Button
        variant="secondary"
        disabled={offset === 0}
        onClick={() => setOffset(Math.max(0, offset - 50))}
      >
        Previous
      </Button>
      <span className="text-sm text-muted">Page {Math.floor(offset / 50) + 1}</span>
      <Button variant="secondary" disabled={count < 50} onClick={() => setOffset(offset + 50)}>
        Next
      </Button>
    </div>
  );
}
function AccountFields({ account }: { account: AdminAccount }) {
  return (
    <>
      <Field label="Name" name="name" defaultValue={account.name} maxLength={80} required />
      <Field label="Email" name="email" type="email" defaultValue={account.email} required />
      <div>
        <label className="label" htmlFor="account-status">
          Account status
        </label>
        <Select
          id="account-status"
          name="status"
          defaultValue={account.deleted_at ? 'deleted' : account.status}
        >
          <option value="active">Active (restores a deleted account)</option>
          <option value="suspended">Suspended</option>
          <option value="deleted">Deleted</option>
        </Select>
      </div>
    </>
  );
}
async function refresh() {
  await queryClient.invalidateQueries({ queryKey: ['admin'] });
}
function UserEditor({ user }: { user: ManagedUser }) {
  const [offset, setOffset] = useState(0);
  const history = useQuery(adminHistoryQuery(user.id, offset));
  const reset = useMutation({ mutationFn: () => adminApi.resetUser(user.id), onSuccess: refresh });
  return (
    <div className="space-y-6">
      <Card className="p-6">
        <h2 className="mb-2 text-lg font-semibold">Edit user</h2>
        <p className="mb-5 text-sm text-muted">
          Changes sign this user out on all devices. Changing their email requires verification of
          the new address. Deletion retains their data and workspace memberships; restoring the
          account restores that access.
        </p>
        <ActionForm
          schema={updateUserSchema}
          label="Save user changes"
          submit={(body) => adminApi.updateUser(user.id, body)}
          onSuccess={refresh}
          successMessage="User updated and sessions revoked."
        >
          <AccountFields account={user} />
        </ActionForm>
        <div className="mt-6 space-y-3 border-t pt-6">
          <p className="text-sm text-muted">
            A password reset revokes sessions and sends a single-use link to {user.email}.
          </p>
          <Button
            variant="secondary"
            disabled={!!user.deleted_at || user.status !== 'active'}
            pending={reset.isPending}
            onClick={() => reset.mutate()}
          >
            Send password reset
          </Button>
          {reset.isError && <ErrorState error={reset.error} />}
          {reset.isSuccess && <Notice kind="success">{reset.data.message}</Notice>}
        </div>
      </Card>
      <Card className="p-6">
        <h2 className="mb-5 text-lg font-semibold">Change history</h2>
        {history.isPending ? (
          <Loading />
        ) : history.isError ? (
          <ErrorState error={history.error} retry={() => void history.refetch()} />
        ) : (
          <>
            {history.data.length === 0 ? (
              <Notice>No recorded changes yet.</Notice>
            ) : (
              <ol className="space-y-5">
                {history.data.map((entry) => (
                  <li key={entry.id} className="space-y-2 border-b pb-5">
                    <p className="text-sm font-medium">
                      {entry.operation === 'INSERT' ? 'Created' : 'Updated'} ·{' '}
                      {new Date(entry.changed_at).toLocaleString()}
                    </p>
                    <p className="break-all text-xs text-muted">
                      {entry.actor_kind}
                      {entry.actor_id ? ` · ${entry.actor_id}` : ''}
                    </p>
                    <dl className="space-y-2 text-sm">
                      {Object.entries(entry.changes).map(([field, change]) => (
                        <div key={field}>
                          <dt className="font-medium">{field}</dt>
                          <dd className="break-all text-muted">
                            {JSON.stringify(change.from)} → {JSON.stringify(change.to)}
                          </dd>
                        </div>
                      ))}
                    </dl>
                  </li>
                ))}
              </ol>
            )}
            <Pager offset={offset} count={history.data.length} setOffset={setOffset} />
          </>
        )}
      </Card>
    </div>
  );
}
function AdminEditor({ account, self }: { account: AdminAccount; self: boolean }) {
  const navigate = useNavigate();
  return (
    <Card className="p-6">
      <h2 className="mb-2 text-lg font-semibold">Edit administrator</h2>
      <p className="mb-5 text-sm text-muted">
        All administrators can manage this installation. Changes revoke this administrator’s
        sessions. You cannot suspend or delete yourself.
      </p>
      <ActionForm
        schema={updateAdminSchema}
        label="Save administrator changes"
        submit={(body) => adminApi.update(account.id, body)}
        onSuccess={async () => {
          if (self) {
            queryClient.removeQueries({ queryKey: ['admin'] });
            await navigate({ to: '/admin/login' });
          } else await refresh();
        }}
        successMessage="Administrator updated and sessions revoked."
      >
        <AccountFields account={account} />
        <Field
          label="New password (optional)"
          name="password"
          type="password"
          autoComplete="new-password"
          hint="Leave blank to keep the current password."
        />
      </ActionForm>
    </Card>
  );
}
export function AdminPage() {
  const navigate = useNavigate();
  const session = useQuery(adminSessionQuery);
  const [tab, setTab] = useState<'users' | 'accounts'>('users');
  const [search, setSearch] = useState('');
  const [offset, setOffset] = useState(0);
  const [selected, setSelected] = useState<string | null>(null);
  const users = useQuery({
    ...adminUsersQuery(search, offset),
    enabled: tab === 'users' && !!session.data,
  });
  const accounts = useQuery({
    ...adminAccountsQuery(search, offset),
    enabled: tab === 'accounts' && !!session.data,
  });
  const logout = useMutation({
    mutationFn: adminApi.logout,
    onSuccess: async () => {
      queryClient.removeQueries({ queryKey: ['admin'] });
      await navigate({ to: '/admin/login' });
    },
  });
  const current = tab === 'users' ? users : accounts;
  const selectedUser = users.data?.find((user) => user.id === selected);
  const selectedAdmin = accounts.data?.find((account) => account.id === selected);
  if (session.isPending) return <Loading />;
  if (session.isError)
    return (
      <main className="p-8">
        <ErrorState error={session.error} />
        <ButtonLink to="/admin/login">Admin sign in</ButtonLink>
      </main>
    );
  return (
    <main className="mx-auto max-w-7xl space-y-7 px-5 py-8 sm:px-8">
      <header className="flex flex-wrap items-center justify-between gap-4">
        <div>
          <p className="eyebrow">ADMINISTRATION</p>
          <p className="mt-2 text-sm text-muted">{session.data.email}</p>
        </div>
        <div className="flex items-center gap-3">
          <ThemeSwitcher />
          <Button variant="secondary" pending={logout.isPending} onClick={() => logout.mutate()}>
            Sign out
          </Button>
        </div>
      </header>
      <PageHeading
        eyebrow="INSTALLATION MANAGEMENT"
        title="Account operations"
        description="Manage access and account details, with a history of every change."
      />
      {logout.isError && <ErrorState error={logout.error} />}
      <nav aria-label="Administration sections" className="flex gap-3">
        {(['users', 'accounts'] as const).map((value) => (
          <Button
            key={value}
            variant={tab === value ? 'primary' : 'secondary'}
            aria-pressed={tab === value}
            onClick={() => {
              setTab(value);
              setOffset(0);
              setSelected(null);
            }}
          >
            {value === 'users' ? 'Application users' : 'Administrators'}
          </Button>
        ))}
      </nav>
      <div className="grid items-start gap-6 lg:grid-cols-2">
        <section className="space-y-6">
          <Card className="p-6">
            <ActionForm
              schema={z.object({ search: z.string().max(254) })}
              label="Search accounts"
              submit={(body) => {
                setSearch(body.search);
                setOffset(0);
                setSelected(null);
                return Promise.resolve();
              }}
            >
              <Field label="Name or email" name="search" type="search" defaultValue={search} />
            </ActionForm>
            <div className="mt-6">
              {current.isPending ? (
                <Loading />
              ) : current.isError ? (
                <ErrorState error={current.error} retry={() => void current.refetch()} />
              ) : (
                <>
                  {current.data.length === 0 ? (
                    <Notice>No matching accounts.</Notice>
                  ) : (
                    <ul className="divide-y">
                      {current.data.map((account) => (
                        <li
                          key={account.id}
                          className="flex flex-wrap items-center justify-between gap-3 py-4"
                        >
                          <div className="min-w-0">
                            <p className="font-medium">{account.name}</p>
                            <p className="break-all text-sm text-muted">{account.email}</p>
                            <Badge>{account.deleted_at ? 'deleted' : account.status}</Badge>
                          </div>
                          <Button
                            variant="secondary"
                            aria-label={`Manage ${account.email}`}
                            aria-pressed={selected === account.id}
                            onClick={() => setSelected(account.id)}
                          >
                            Manage
                          </Button>
                        </li>
                      ))}
                    </ul>
                  )}
                  <Pager
                    offset={offset}
                    count={current.data.length}
                    setOffset={(value) => {
                      setOffset(value);
                      setSelected(null);
                    }}
                  />
                </>
              )}
            </div>
          </Card>
          {tab === 'accounts' && (
            <Card className="p-6">
              <h2 className="mb-5 text-lg font-semibold">Add administrator</h2>
              <ActionForm
                schema={createAdminSchema}
                label="Create administrator"
                submit={adminApi.create}
                onSuccess={refresh}
                successMessage="Administrator created. Share their credentials securely."
              >
                <Field label="Name" name="name" maxLength={80} required />
                <Field label="Email" name="email" type="email" required />
                <Field
                  label="Initial password"
                  name="password"
                  type="password"
                  autoComplete="new-password"
                  minLength={12}
                  required
                />
              </ActionForm>
            </Card>
          )}
        </section>
        <section aria-label="Account details">
          {tab === 'users' && selectedUser ? (
            <UserEditor key={selectedUser.id} user={selectedUser} />
          ) : tab === 'accounts' && selectedAdmin ? (
            <AdminEditor
              key={selectedAdmin.id}
              account={selectedAdmin}
              self={selectedAdmin.id === session.data.id}
            />
          ) : (
            <Card className="p-8">
              <Notice>Select an account to manage its details.</Notice>
            </Card>
          )}
        </section>
      </div>
    </main>
  );
}
