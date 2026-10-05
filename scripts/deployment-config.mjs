import { readFileSync } from 'node:fs';
import { z } from 'zod';

export const configPath = 'appshell.deploy.json';
const name = z.string().regex(/^[a-z][a-z0-9-]{1,39}$/);
const region = z.string().regex(/^[a-z]{2}-[a-z]+-\d$/);
const secretArn = z
  .string()
  .regex(/^arn:aws:secretsmanager:[a-z0-9-]+:\d{12}:secret:[A-Za-z0-9/_+=.@-]+$/);
const roleArn = z.string().regex(/^arn:aws:iam::\d{12}:role\/[A-Za-z0-9/+=,.@_-]+$/);
const common = { version: z.literal(1), name, autoDeploy: z.boolean().default(false) };
export const deploymentSchema = z
  .discriminatedUnion('mode', [
    z.strictObject({
      ...common,
      mode: z.literal('docker'),
      storage: z.literal('postgres').default('postgres'),
      jobs: z.enum(['disabled', 'postgres']).default('disabled'),
      workerServiceId: z
        .string()
        .regex(/^srv-[a-z0-9]+$/)
        .optional(),
      serviceId: z
        .string()
        .regex(/^srv-[a-z0-9]+$/)
        .optional(),
    }),
    z.strictObject({
      ...common,
      mode: z.literal('lambda'),
      architecture: z.enum(['x86_64', 'arm64']).default('x86_64'),
      storage: z.enum(['postgres', 'dynamodb']).default('postgres'),
      jobs: z.enum(['disabled', 'sqs']).default('disabled'),
      region,
      secretArn: secretArn.optional(),
      roleArn: roleArn.optional(),
      mailFrom: z
        .string()
        .min(3)
        .max(200)
        .regex(/^[^\r\n]+@[^\r\n]+$/)
        .optional(),
      mailBrand: z
        .string()
        .min(1)
        .max(100)
        .regex(/^[^\r\n]+$/),
      mailSchedule: z
        .enum(['rate(1 minute)', 'rate(5 minutes)', 'rate(15 minutes)'])
        .default('rate(1 minute)'),
    }),
  ])
  .superRefine((value, ctx) => {
    if (value.mode === 'docker' && value.workerServiceId === value.serviceId && value.serviceId)
      ctx.addIssue({
        code: 'custom',
        message: 'The worker and web service must be separate Render services',
        path: ['workerServiceId'],
      });
    if (value.mode === 'lambda' && value.secretArn) {
      const parts = value.secretArn.split(':');
      if (parts[3] !== value.region)
        ctx.addIssue({
          code: 'custom',
          message: 'Secret ARN must use the deployment region',
          path: ['secretArn'],
        });
      if (value.roleArn && value.roleArn.split(':')[4] !== parts[4])
        ctx.addIssue({
          code: 'custom',
          message: 'Role and secret must use the same AWS account',
          path: ['roleArn'],
        });
    }
  });

export function readConfig(file = configPath) {
  try {
    return deploymentSchema.parse(JSON.parse(readFileSync(file, 'utf8')));
  } catch (error) {
    if (error.code === 'ENOENT') throw new Error(`Run npm run setup first (${file} is missing).`);
    throw error;
  }
}

export function missingSettings(config, env = process.env) {
  if (config.mode === 'docker') {
    return [
      !config.serviceId && 'serviceId (run setup after creating the Render service)',
      !env.RENDER_API_KEY && 'RENDER_API_KEY environment variable',
      config.jobs === 'postgres' &&
        !config.workerServiceId &&
        'workerServiceId (separate Render worker)',
    ].filter(Boolean);
  }
  return [
    config.storage !== 'dynamodb' &&
      !config.secretArn &&
      'secretArn (existing Secrets Manager secret)',
    !config.mailFrom && 'mailFrom (verified sender)',
  ].filter(Boolean);
}

export function deploymentPlan(config) {
  if (config.mode === 'docker')
    return [
      ...(config.jobs === 'postgres'
        ? [
            `Deploy shared Postgres worker ${config.workerServiceId ?? '(not configured)'} first. Configure deploy/render.jobs.yaml: JOB_BACKEND=postgres on both services; RUN_MODE=api on web, worker on worker.`,
          ]
        : []),
      `Deploy the current Git commit to Render service ${config.serviceId ?? '(not configured)'}.`,
      'Render builds Dockerfile and runs startup migrations; wait until the deployment is live.',
    ];
  return [
    `Use AWS ${config.region}; stack and ECR repository: ${config.name}.`,
    'Build the frontend locally; create the ECR repository if missing and push the Lambda image.',
    'Create/update the SAM stack; discover and configure its CloudFront origin automatically.',
    config.storage === 'dynamodb'
      ? 'Provision on-demand DynamoDB with atomic history and SES; validate its schema before publishing.'
      : 'Invoke migrations and check their result before uploading any frontend files.',
    'Upload assets, publish index.html last, then invalidate CloudFront.',
    config.jobs === 'sqs'
      ? 'Enable SQS jobs and a recovery dispatcher; mail delivery is triggered by SQS.'
      : `Mail delivery schedule: ${config.mailSchedule}.`,
  ];
}
