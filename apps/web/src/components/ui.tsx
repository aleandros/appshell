import { useId, useState } from 'react';
import type { ButtonHTMLAttributes, InputHTMLAttributes, ReactNode } from 'react';
import { useMutation } from '@tanstack/react-query';
import { AlertCircle, ArrowRight, CheckCircle2, LoaderCircle, Sprout } from 'lucide-react';
import { z } from 'zod';
import { clsx } from 'clsx';
import { errorMessage } from '../lib/errors';
import { brand } from '../config/brand';

export function Logo({ compact = false }: { compact?: boolean }) {
  return (
    <span className="inline-flex items-center gap-2.5 text-xl font-semibold tracking-tight">
      <span className="flex h-9 w-9 items-center justify-center rounded-xl bg-primary text-primary-foreground">
        <Sprout size={21} strokeWidth={2.2} />
      </span>
      {!compact && brand.name}
    </span>
  );
}
export function Button({
  children,
  pending = false,
  variant = 'primary',
  className,
  disabled,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  pending?: boolean;
  variant?: 'primary' | 'secondary' | 'quiet';
}) {
  return (
    <button
      {...props}
      disabled={disabled || pending}
      aria-busy={pending}
      className={clsx('btn', `btn-${variant}`, className)}
    >
      {pending && <LoaderCircle size={16} className="animate-spin" aria-hidden="true" />}
      {children}
    </button>
  );
}
export function Field({
  label,
  hint,
  ...props
}: InputHTMLAttributes<HTMLInputElement> & { label: string; hint?: string }) {
  const id = useId();
  return (
    <div>
      <label htmlFor={id} className="label">
        {label}
      </label>
      <input
        id={id}
        className="input"
        {...props}
        aria-describedby={hint ? `${id}-hint` : undefined}
      />
      {hint && (
        <p id={`${id}-hint`} className="mt-2 text-xs text-muted">
          {hint}
        </p>
      )}
    </div>
  );
}
export function Notice({
  children,
  kind = 'info',
}: {
  children: ReactNode;
  kind?: 'error' | 'success' | 'info';
}) {
  const Icon = kind === 'success' ? CheckCircle2 : AlertCircle;
  return (
    <div
      role={kind === 'error' ? 'alert' : 'status'}
      className={clsx(
        'flex items-start gap-3 rounded-xl border p-4 text-sm leading-relaxed',
        kind === 'error' ? 'bg-danger-muted text-danger' : 'bg-accent/50 text-accent-foreground',
      )}
    >
      <Icon className="mt-0.5 shrink-0" size={17} />
      <div>{children}</div>
    </div>
  );
}
export function Loading({ label = 'Loading your workspace…' }: { label?: string }) {
  return (
    <div role="status" className="space-y-5 p-6">
      <span className="sr-only">{label}</span>
      <div className="h-8 w-52 animate-pulse rounded-lg bg-surface-muted" />
      <div className="grid gap-4 sm:grid-cols-3">
        {[1, 2, 3].map((i) => (
          <div key={i} className="h-36 animate-pulse rounded-2xl bg-surface-muted" />
        ))}
      </div>
      <div className="h-64 animate-pulse rounded-2xl bg-surface-muted" />
    </div>
  );
}
export function ErrorState({ error, retry }: { error: unknown; retry?: () => void }) {
  return (
    <div className="mx-auto max-w-lg space-y-5 py-12">
      <Notice kind="error">{errorMessage(error)}</Notice>
      {retry && (
        <Button variant="secondary" onClick={retry}>
          Try again
        </Button>
      )}
    </div>
  );
}
export function Badge({ children }: { children: ReactNode }) {
  return (
    <span className="inline-flex items-center gap-1.5 rounded-full bg-accent px-2.5 py-1 text-[11px] font-semibold capitalize text-accent-foreground">
      {children}
    </span>
  );
}
export function PageHeading({
  eyebrow,
  title,
  description,
  action,
}: {
  eyebrow: string;
  title: string;
  description: string;
  action?: ReactNode;
}) {
  return (
    <div className="mb-8 flex flex-wrap items-end justify-between gap-4">
      <div>
        <p className="eyebrow mb-2.5">{eyebrow}</p>
        <h1 className="page-title">{title}</h1>
        <p className="mt-3 text-muted">{description}</p>
      </div>
      {action}
    </div>
  );
}
export function Plant() {
  return (
    <div className="botanical" aria-hidden="true">
      <span className="stem" />
      <span className="leaf" />
      <span className="leaf" />
      <span className="leaf" />
      <span className="pot" />
    </div>
  );
}
export function ActionForm<S extends z.ZodType>({
  schema,
  submit,
  children,
  label,
  onSuccess,
  successMessage,
  className,
}: {
  schema: S;
  submit: (data: z.output<S>) => Promise<unknown>;
  children: ReactNode;
  label: string;
  onSuccess?: () => void | Promise<void>;
  successMessage?: string;
  className?: string;
}) {
  const [validation, setValidation] = useState<string | null>(null);
  const mutation = useMutation({
    mutationFn: submit,
    onSuccess: async () => {
      await onSuccess?.();
    },
  });
  return (
    <form
      className={clsx('space-y-5', className)}
      onSubmit={(event) => {
        event.preventDefault();
        const parsed = schema.safeParse(Object.fromEntries(new FormData(event.currentTarget)));
        if (!parsed.success) {
          setValidation(
            parsed.error.issues
              .map((issue) => `${issue.path.join(' ')}: ${issue.message}`)
              .join(' '),
          );
          return;
        }
        setValidation(null);
        mutation.mutate(parsed.data);
      }}
    >
      <fieldset disabled={mutation.isPending} className="min-w-0 space-y-5">
        {children}
      </fieldset>
      {validation && <Notice kind="error">{validation}</Notice>}
      {mutation.isError && <Notice kind="error">{errorMessage(mutation.error)}</Notice>}
      {mutation.isSuccess && successMessage && <Notice kind="success">{successMessage}</Notice>}
      <Button type="submit" pending={mutation.isPending} className="w-full">
        {label}
        <ArrowRight size={16} />
      </Button>
    </form>
  );
}
