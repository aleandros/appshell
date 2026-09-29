import { useId } from 'react';
import type {
  AnchorHTMLAttributes,
  ButtonHTMLAttributes,
  InputHTMLAttributes,
  SelectHTMLAttributes,
  HTMLAttributes,
  ReactNode,
  Ref,
} from 'react';
import { AlertCircle, CheckCircle2, LoaderCircle, Sprout } from 'lucide-react';
import { clsx } from 'clsx';
export function Logo({ compact = false, name }: { compact?: boolean; name: string }) {
  return (
    <span className="inline-flex items-center gap-2.5 text-xl font-semibold tracking-tight">
      <span className="flex h-9 w-9 items-center justify-center rounded-xl bg-primary text-primary-foreground">
        <Sprout size={21} strokeWidth={2.2} />
      </span>
      {!compact && name}
    </span>
  );
}
export function Button({
  children,
  pending = false,
  variant = 'primary',
  className,
  disabled,
  type = 'button',
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  pending?: boolean;
  variant?: 'primary' | 'secondary' | 'quiet';
}) {
  return (
    <button
      {...props}
      type={type}
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
  id: suppliedId,
  className,
  'aria-describedby': describedBy,
  ...props
}: InputHTMLAttributes<HTMLInputElement> & { label: string; hint?: string }) {
  const generatedId = useId();
  const id = suppliedId ?? generatedId;
  return (
    <div>
      <label htmlFor={id} className="label">
        {label}
      </label>
      <input
        {...props}
        id={id}
        className={clsx('input', className)}
        aria-describedby={
          [describedBy, hint ? `${id}-hint` : undefined].filter(Boolean).join(' ') || undefined
        }
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

export function Select({ className, ...props }: SelectHTMLAttributes<HTMLSelectElement>) {
  return <select {...props} className={clsx('input', className)} />;
}
export function Card({
  as: Tag = 'section',
  className,
  ...props
}: HTMLAttributes<HTMLElement> & { as?: 'section' | 'article' | 'div' }) {
  return <Tag {...props} className={clsx('card', className)} />;
}
export function ButtonAnchor({
  variant = 'primary',
  className,
  ref,
  ...props
}: AnchorHTMLAttributes<HTMLAnchorElement> & {
  variant?: 'primary' | 'secondary' | 'quiet';
  ref?: Ref<HTMLAnchorElement>;
}) {
  return <a {...props} ref={ref} className={clsx('btn', `btn-${variant}`, className)} />;
}
export function NavigationAnchor({
  className,
  ref,
  ...props
}: AnchorHTMLAttributes<HTMLAnchorElement> & { ref?: Ref<HTMLAnchorElement> }) {
  return <a {...props} ref={ref} className={clsx('nav-item', className)} />;
}
export function CardAnchor({
  className,
  ref,
  ...props
}: AnchorHTMLAttributes<HTMLAnchorElement> & { ref?: Ref<HTMLAnchorElement> }) {
  return <a {...props} ref={ref} className={clsx('card', className)} />;
}
export function ErrorState({ message, retry }: { message: string; retry?: () => void }) {
  return (
    <div className="mx-auto max-w-lg space-y-5 py-12">
      <Notice kind="error">{message}</Notice>
      {retry && (
        <Button variant="secondary" onClick={retry}>
          Try again
        </Button>
      )}
    </div>
  );
}
