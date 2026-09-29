import { queryOptions } from '@tanstack/react-query';
import { api } from '../../lib/api';
import { sessionSchema, messageSchema } from '../../lib/schemas';
import type { components } from '../../lib/api.generated';
type Input = components['schemas'];
export const sessionQuery = queryOptions({
  queryKey: ['session'],
  queryFn: ({ signal }) => api('/auth/session', sessionSchema, { signal }),
  staleTime: 0,
  retry: false,
});

export const identityApi = {
  signup: (body: Input['Signup']) => api('/auth/signup', sessionSchema, { method: 'POST', body }),
  login: (body: Input['Login']) => api('/auth/login', sessionSchema, { method: 'POST', body }),
  logout: () => api('/auth/logout', messageSchema, { method: 'POST' }),
  forgotPassword: (body: Input['EmailInput']) =>
    api('/auth/forgot-password', messageSchema, { method: 'POST', body }),
  resendVerification: () => api('/auth/resend-verification', messageSchema, { method: 'POST' }),
  changePassword: (body: Input['ChangePassword']) =>
    api('/auth/change-password', messageSchema, { method: 'POST', body }),
  changeEmail: (body: Input['ChangeEmail']) =>
    api('/auth/change-email', messageSchema, { method: 'POST', body }),
  completeAction: (
    flow: 'verify-email' | 'reset-password' | 'confirm-email',
    body: { token: string; password?: string },
  ) => api(`/auth/${flow}`, messageSchema, { method: 'POST', body }),
};
