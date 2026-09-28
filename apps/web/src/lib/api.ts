import { queryOptions, QueryClient } from '@tanstack/react-query';
import { z } from 'zod';
import { sessionSchema } from './schemas';
export class ApiError extends Error {
  constructor(
    public readonly status: number,
    public readonly code: string,
    message: string,
  ) {
    super(message);
    this.name = 'ApiError';
  }
}
export async function api<T>(
  path: string,
  schema: z.ZodType<T>,
  options: { method?: 'GET' | 'POST' | 'DELETE'; body?: unknown; signal?: AbortSignal } = {},
): Promise<T> {
  const base = import.meta.env.VITE_API_URL as string | undefined;
  let response: Response;
  try {
    response = await fetch(`${base ?? ''}/api${path}`, {
      method: options.method ?? 'GET',
      credentials: 'include',
      headers: { 'Content-Type': 'application/json' },
      ...(options.body !== undefined ? { body: JSON.stringify(options.body) } : {}),
      ...(options.signal ? { signal: options.signal } : {}),
    });
  } catch (error) {
    if (error instanceof DOMException && error.name === 'AbortError') throw error;
    throw new ApiError(
      0,
      'offline',
      'We couldn’t reach the server. Check your connection and try again.',
    );
  }
  const payload: unknown = await response.json().catch(() => null);
  if (!response.ok) {
    const error = z.object({ code: z.string(), message: z.string() }).safeParse(payload);
    throw new ApiError(
      response.status,
      error.success ? error.data.code : 'request_failed',
      error.success ? error.data.message : 'The request failed. Please try again.',
    );
  }
  const result = schema.safeParse(payload);
  if (!result.success)
    throw new ApiError(
      502,
      'invalid_response',
      'The server returned an unexpected response. Please refresh or try again.',
    );
  return result.data;
}
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
export const sessionQuery = queryOptions({
  queryKey: ['session'],
  queryFn: ({ signal }) => api('/auth/session', sessionSchema, { signal }),
  staleTime: 0,
  retry: false,
});
export function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'Something went wrong. Please try again.';
}
