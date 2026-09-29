import { queryOptions } from '@tanstack/react-query';
import { z } from 'zod';
import { api } from '../../lib/api';
import { email, name, password, messageSchema } from '../../lib/schemas';
import type { components } from '../../lib/api.generated';
type Schema = components['schemas'];
const accountSchema = z.object({
  id: z.uuid(),
  email: z.email(),
  name: z.string(),
  status: z.enum(['active', 'suspended']),
  deleted_at: z.string().nullable(),
}) satisfies z.ZodType<Schema['AdminAccount']>;
const userSchema = accountSchema.extend({
  email_verified_at: z.string().nullable(),
}) satisfies z.ZodType<Schema['ManagedUser']>;
const historySchema = z.object({
  id: z.uuid(),
  operation: z.enum(['INSERT', 'UPDATE']),
  changes: z.record(z.string(), z.object({ from: z.unknown(), to: z.unknown() })),
  actor_id: z.uuid().nullable(),
  actor_kind: z.enum(['user', 'admin', 'system']),
  changed_at: z.string(),
}) satisfies z.ZodType<Schema['HistoryEntry']>;
export type AdminAccount = z.infer<typeof accountSchema>;
export type ManagedUser = z.infer<typeof userSchema>;
export const createAdminSchema = z.object({ email, name, password }) satisfies z.ZodType<
  Schema['CreateAdmin']
>;
export const updateUserSchema = z.object({
  email,
  name,
  status: z.enum(['active', 'suspended', 'deleted']),
}) satisfies z.ZodType<Schema['UpdateUser']>;
export const updateAdminSchema = updateUserSchema.extend({
  password: z.union([z.literal(''), password]).transform((value) => value || null),
}) satisfies z.ZodType<Schema['UpdateAdmin']>;
export const adminSessionQuery = queryOptions({
  queryKey: ['admin', 'session'],
  queryFn: ({ signal }) => api('/admin/session', accountSchema, { signal }),
  staleTime: 0,
  retry: false,
});
export const adminUsersQuery = (search: string, offset: number) =>
  queryOptions({
    queryKey: ['admin', 'users', search, offset],
    queryFn: ({ signal }) =>
      api(
        `/admin/users?${new URLSearchParams({ search, offset: String(offset) })}`,
        z.array(userSchema),
        { signal },
      ),
  });
export const adminAccountsQuery = (search: string, offset: number) =>
  queryOptions({
    queryKey: ['admin', 'accounts', search, offset],
    queryFn: ({ signal }) =>
      api(
        `/admin/accounts?${new URLSearchParams({ search, offset: String(offset) })}`,
        z.array(accountSchema),
        { signal },
      ),
  });
export const adminHistoryQuery = (id: string, offset: number) =>
  queryOptions({
    queryKey: ['admin', 'history', id, offset],
    queryFn: ({ signal }) =>
      api(`/admin/users/${id}/history?offset=${String(offset)}`, z.array(historySchema), {
        signal,
      }),
  });
export const adminApi = {
  login: (body: Schema['Login']) => api('/admin/login', accountSchema, { method: 'POST', body }),
  logout: () => api('/admin/logout', messageSchema, { method: 'POST' }),
  create: (body: Schema['CreateAdmin']) =>
    api('/admin/accounts', accountSchema, { method: 'POST', body }),
  update: (id: string, body: Schema['UpdateAdmin']) =>
    api(`/admin/accounts/${id}`, messageSchema, { method: 'POST', body }),
  updateUser: (id: string, body: Schema['UpdateUser']) =>
    api(`/admin/users/${id}`, messageSchema, { method: 'POST', body }),
  resetUser: (id: string) =>
    api(`/admin/users/${id}/reset-password`, messageSchema, { method: 'POST' }),
};
