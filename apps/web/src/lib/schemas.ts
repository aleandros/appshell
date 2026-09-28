import { z } from 'zod';
import type { components } from './api.generated';
type Schema = components['schemas'];
export const userSchema: z.ZodType<Schema['User']> = z.object({
  id: z.uuid(),
  name: z.string(),
  email: z.email(),
  email_verified_at: z.string().nullable(),
});
export const organizationSchema: z.ZodType<Schema['Organization']> = z.object({
  id: z.uuid(),
  name: z.string(),
  role: z.enum(['owner', 'admin', 'member']),
});
export const sessionSchema = z.object({
  user: userSchema,
  organizations: z.array(organizationSchema),
}) satisfies z.ZodType<Schema['Session']>;
export type Session = z.infer<typeof sessionSchema>;
export type Organization = Schema['Organization'];
export const messageSchema = z.object({ message: z.string() }) satisfies z.ZodType<
  Schema['Message']
>;
export const teamSchema = z.object({
  members: z.array(
    z.object({
      id: z.uuid(),
      name: z.string(),
      email: z.email(),
      role: z.enum(['owner', 'admin', 'member']),
    }),
  ),
  invitations: z.array(
    z.object({
      id: z.uuid(),
      email: z.email(),
      role: z.enum(['admin', 'member']),
      expires_at: z.string(),
    }),
  ),
}) satisfies z.ZodType<Schema['Team']>;
export const billingSchema = z.object({
  subscription: z.object({
    plan: z.enum(['free', 'pro']),
    status: z.string(),
    current_period_end: z.string().nullable(),
  }),
  seat_limit: z.number(),
  seats_used: z.number(),
  billing_enabled: z.boolean(),
}) satisfies z.ZodType<Schema['Billing']>;
export const redirectSchema = z.object({ url: z.url() }) satisfies z.ZodType<Schema['RedirectUrl']>;
export const email = z
  .email('Enter a valid email address.')
  .max(254)
  .transform((value) => value.trim().toLowerCase());
export const password = z
  .string()
  .min(12, 'Use at least 12 characters.')
  .refine((value) => new TextEncoder().encode(value).length <= 128, 'Use at most 128 bytes.');
export const name = z.string().trim().min(1, 'This field is required.').max(80);
export const signupSchema = z.object({
  name,
  email,
  password,
  organization: name,
}) satisfies z.ZodType<Schema['Signup']>;
export const loginSchema = z.object({
  email,
  password: z.string().min(1, 'Enter your password.'),
}) satisfies z.ZodType<Schema['Login']>;
