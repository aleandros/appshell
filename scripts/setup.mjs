import { existsSync, writeFileSync } from 'node:fs';
import { createInterface } from 'node:readline/promises';
import { parseArgs } from 'node:util';
import { configPath, deploymentSchema, readConfig } from './deployment-config.mjs';

const { values } = parseArgs({
  options: {
    mode: { type: 'string' },
    jobs: { type: 'string' },
    'worker-service-id': { type: 'string' },
    name: { type: 'string' },
    region: { type: 'string' },
    'service-id': { type: 'string' },
    'secret-arn': { type: 'string' },
    'role-arn': { type: 'string' },
    'mail-from': { type: 'string' },
    'mail-brand': { type: 'string' },
    'mail-schedule': { type: 'string' },
    'auto-deploy': { type: 'boolean' },
    'no-auto-deploy': { type: 'boolean' },
    help: { type: 'boolean' },
  },
});
if (values.help) {
  console.log(
    'npm run setup [-- --mode docker|lambda --name my-app --region us-east-1]\nOptional: --jobs disabled|postgres|sqs, --worker-service-id, --service-id, --secret-arn, --role-arn, --mail-from, --mail-brand, --mail-schedule, --auto-deploy, --no-auto-deploy.\nSupplying --mode selects noninteractive setup. This only saves settings; no cloud resources are created.',
  );
  process.exit(0);
}
if (values['auto-deploy'] && values['no-auto-deploy'])
  throw new Error('Choose one auto-deploy option.');
const previous = existsSync(configPath) ? readConfig() : undefined;
const interactive = !values.mode;
if (interactive && !process.stdin.isTTY)
  throw new Error('Use --mode docker|lambda for noninteractive setup.');
const rl = interactive
  ? createInterface({ input: process.stdin, output: process.stdout })
  : undefined;
async function field(flag, label, fallback = '') {
  if (values[flag] !== undefined) return values[flag];
  if (!rl) return fallback || undefined;
  return (
    (await rl.question(`${label}${fallback ? ` [${fallback}]` : ' (optional)'}: `)).trim() ||
    fallback ||
    undefined
  );
}
try {
  const mode = await field('mode', 'Deployment mode: docker or lambda', previous?.mode ?? 'docker');
  if (!['docker', 'lambda'].includes(mode)) throw new Error('Mode must be docker or lambda.');
  const matching = previous?.mode === mode ? previous : undefined;
  const appName = await field(
    'name',
    'App name (lowercase letters, numbers, hyphens)',
    previous?.name ?? 'my-app',
  );
  let autoDeploy =
    values['auto-deploy'] ?? (values['no-auto-deploy'] ? false : (previous?.autoDeploy ?? false));
  if (rl && values['auto-deploy'] === undefined && values['no-auto-deploy'] === undefined) {
    const answer = await rl.question(
      `Deploy automatically after checks on the default branch? [${autoDeploy ? 'Y/n' : 'y/N'}]: `,
    );
    if (answer.trim()) {
      if (!/^(y|yes|n|no)$/i.test(answer.trim())) throw new Error('Answer yes or no.');
      autoDeploy = /^y/i.test(answer.trim());
    }
  }
  const jobs = await field(
    'jobs',
    mode === 'docker'
      ? 'Background jobs: disabled or postgres'
      : 'Background jobs: disabled or sqs',
    matching?.jobs ?? 'disabled',
  );
  const common = { version: 1, mode, name: appName, autoDeploy, jobs };
  const config = deploymentSchema.parse(
    mode === 'docker'
      ? {
          ...common,
          workerServiceId:
            jobs === 'postgres'
              ? await field(
                  'worker-service-id',
                  'Render background worker service ID (can add later)',
                  matching?.workerServiceId,
                )
              : undefined,
          serviceId: await field(
            'service-id',
            'Render service ID (can add later)',
            matching?.serviceId,
          ),
        }
      : {
          ...common,
          region: await field('region', 'AWS region', matching?.region ?? 'us-east-1'),
          secretArn: await field(
            'secret-arn',
            'Secrets Manager ARN (can add later)',
            matching?.secretArn,
          ),
          roleArn: await field(
            'role-arn',
            'GitHub OIDC deployment role ARN (can add later)',
            matching?.roleArn,
          ),
          mailFrom: await field(
            'mail-from',
            'Verified sender, e.g. My App <hello@example.com>',
            matching?.mailFrom,
          ),
          mailBrand: await field('mail-brand', 'Email brand', matching?.mailBrand ?? appName),
          mailSchedule: await field(
            'mail-schedule',
            'Mail polling: rate(1 minute), rate(5 minutes), rate(15 minutes)',
            matching?.mailSchedule ?? 'rate(1 minute)',
          ),
        },
  );
  writeFileSync(configPath, JSON.stringify(config, null, 2) + '\n');
  console.log(
    `Saved ${configPath}. Commit this non-secret configuration with your app.\nRun npm run deploy:plan for prerequisites and the release steps.\nLocal development: ${jobs === 'disabled' ? 'docker compose up -d db api' : 'docker compose -f compose.yaml -f compose.jobs.yaml up -d db api worker'} && npm run dev`,
  );
} finally {
  rl?.close();
}
