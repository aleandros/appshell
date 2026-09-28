import { afterEach, describe, expect, it, vi } from 'vitest';
import { z } from 'zod';
import { api } from './api';
import { signupSchema } from './schemas';
afterEach(() => vi.unstubAllGlobals());
describe('API boundary', () => {
  it('validates response payloads instead of trusting a successful status', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue(new Response(JSON.stringify({ count: 'invalid' }))),
    );
    await expect(api('/test', z.object({ count: z.number() }))).rejects.toMatchObject({
      code: 'invalid_response',
    });
  });
  it('preserves structured permission errors', async () => {
    vi.stubGlobal(
      'fetch',
      vi
        .fn()
        .mockResolvedValue(
          new Response(JSON.stringify({ code: 'forbidden', message: 'Not allowed' }), {
            status: 403,
          }),
        ),
    );
    await expect(api('/test', z.object({}))).rejects.toMatchObject({
      status: 403,
      message: 'Not allowed',
    });
  });
  it('handles offline and non-JSON server failures', async () => {
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new TypeError('Failed to fetch')));
    await expect(api('/test', z.object({}))).rejects.toMatchObject({ code: 'offline' });
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response('Bad gateway', { status: 502 })));
    await expect(api('/test', z.object({}))).rejects.toMatchObject({
      status: 502,
      code: 'request_failed',
    });
  });
  it('rejects weak and excessively large passwords before submission', () => {
    const input = {
      email: 'person@example.com',
      name: 'Person',
      organization: 'Studio',
      password: 'short',
    };
    expect(signupSchema.safeParse(input).success).toBe(false);
    expect(signupSchema.safeParse({ ...input, password: 'a'.repeat(129) }).success).toBe(false);
    expect(
      signupSchema.safeParse({ ...input, password: 'a sufficiently long password' }).success,
    ).toBe(true);
  });
});
