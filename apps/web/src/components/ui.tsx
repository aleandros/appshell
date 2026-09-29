import { useState } from 'react';
import type { ReactNode } from 'react';
import { useMutation } from '@tanstack/react-query';
import { ArrowRight } from 'lucide-react';
import { z } from 'zod';
import { clsx } from 'clsx';
import { errorMessage } from '../lib/errors';
import { brand } from '../config/brand';
import { Button, Notice, Logo as SharedLogo, ErrorState as SharedErrorState } from '@appshell/ui';
export {
  Button,
  Field,
  Notice,
  Loading,
  Badge,
  PageHeading,
  Plant,
  Select,
  Card,
  ButtonAnchor,
} from '@appshell/ui';
export function Logo({ compact = false }: { compact?: boolean }) {
  return <SharedLogo name={brand.name} compact={compact} />;
}
export function ErrorState({ error, retry }: { error: unknown; retry?: () => void }) {
  return <SharedErrorState message={errorMessage(error)} {...(retry ? { retry } : {})} />;
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
