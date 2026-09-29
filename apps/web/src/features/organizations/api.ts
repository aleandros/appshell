import { queryOptions } from '@tanstack/react-query';
import { api } from '../../lib/api';
import { teamSchema, organizationSchema, messageSchema } from '../../lib/schemas';
import type { components } from '../../lib/api.generated';
type Input = components['schemas'];
export const teamQuery = (organizationId: string) =>
  queryOptions({
    queryKey: ['team', organizationId],
    queryFn: ({ signal }) => api(`/organizations/${organizationId}/team`, teamSchema, { signal }),
  });
export const organizationsApi = {
  create: (body: Input['NameInput']) =>
    api('/organizations', organizationSchema, { method: 'POST', body }),
  invite: (organizationId: string, body: Input['InviteInput']) =>
    api(`/organizations/${organizationId}/invitations`, messageSchema, { method: 'POST', body }),
  remove: (organizationId: string, kind: 'members' | 'invitations', id: string) =>
    api(`/organizations/${organizationId}/${kind}/${id}`, messageSchema, { method: 'DELETE' }),
  accept: (token: string) =>
    api('/invitations/accept', messageSchema, { method: 'POST', body: { token } }),
};
