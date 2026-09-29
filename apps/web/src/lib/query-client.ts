import { QueryClient } from '@tanstack/react-query';
import { ApiError } from './errors';
export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      retry: (attempt, error) =>
        !(error instanceof ApiError && error.status >= 400 && error.status < 500) && attempt < 1,
      refetchOnWindowFocus: true,
    },
    mutations: { retry: false },
  },
});
