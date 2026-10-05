import { useTranslation } from 'react-i18next';
import { useState } from 'react';
import type { ReactNode } from 'react';
import { useMutation } from '@tanstack/react-query';
import { ArrowRight } from 'lucide-react';
import { z } from 'zod';
import { clsx } from 'clsx';
import { errorMessage } from '../lib/errors';
import { brand } from '../config/brand';
import {
  Button,
  Notice,
  Logo as SharedLogo,
  Loading as SharedLoading,
  ErrorState as SharedErrorState,
} from '@appshell/ui';
export {
  Button,
  Field,
  Notice,
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
  const { t } = useTranslation();
  return (
    <SharedErrorState
      retryLabel={t('Try again')}
      message={errorMessage(error, t)}
      {...(retry ? { retry } : {})}
    />
  );
}

export function Loading({ label }: { label?: string }) {
  const { t } = useTranslation();
  return <SharedLoading label={label ?? t('Loading your workspace…')} />;
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
  const { t } = useTranslation();
  const [validation, setValidation] = useState<z.core.$ZodIssue[]>([]);
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
          setValidation(parsed.error.issues);
          return;
        }
        setValidation([]);
        mutation.mutate(parsed.data);
      }}
    >
      <fieldset disabled={mutation.isPending} className="min-w-0 space-y-5">
        {children}
      </fieldset>
      {validation.length > 0 && (
        <Notice kind="error">
          {validation
            .map((issue) =>
              t(issue.message, { defaultValue: t('Please check the submitted fields.') }),
            )
            .join(' ')}
        </Notice>
      )}
      {mutation.isError && <Notice kind="error">{errorMessage(mutation.error, t)}</Notice>}
      {mutation.isSuccess && successMessage && <Notice kind="success">{successMessage}</Notice>}
      <Button type="submit" pending={mutation.isPending} className="w-full">
        {label}
        <ArrowRight size={16} />
      </Button>
    </form>
  );
}
