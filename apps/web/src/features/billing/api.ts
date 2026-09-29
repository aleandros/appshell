import { queryOptions } from '@tanstack/react-query';
import { api } from '../../lib/api';
import { billingSchema, redirectSchema } from '../../lib/schemas';
export const billingQuery = (organizationId: string) =>
  queryOptions({
    queryKey: ['billing', organizationId],
    queryFn: ({ signal }) =>
      api(`/organizations/${organizationId}/billing`, billingSchema, { signal }),
  });
export const billingApi = {
  openPortal: (organizationId: string, mode: 'checkout' | 'billing-portal') =>
    api(`/organizations/${organizationId}/${mode}`, redirectSchema, { method: 'POST' }),
};
