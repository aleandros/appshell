import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { setTimeout } from 'node:timers/promises';
import { z } from 'zod';

// Argument arrays, never shell interpolation. Credentials only travel via the
// environment or stdin, and captured output is not echoed automatically.
export function command(executable, args, options = {}) {
  const { capture = false, input, ...rest } = options;
  const result = spawnSync(executable, args, {
    encoding: 'utf8',
    maxBuffer: 10 * 1024 * 1024,
    stdio: capture
      ? ['pipe', 'pipe', 'pipe']
      : [input === undefined ? 'ignore' : 'pipe', 'inherit', 'inherit'],
    ...rest,
    input,
  });
  if (result.status !== 0)
    throw new Error(
      `${executable} ${args[0]} failed: ${result.stderr || result.error?.message || result.status}`,
    );
  return capture ? result.stdout.trim() : '';
}

export function releaseCommit(run = command) {
  if (run('git', ['status', '--porcelain'], { capture: true }))
    throw new Error(
      'Commit or stash working-tree changes before deploying. The release must match a Git commit.',
    );
  const sha = run('git', ['rev-parse', 'HEAD'], { capture: true });
  if (!/^[a-f0-9]{40}$/.test(sha)) throw new Error('Expected a full Git commit SHA.');
  return sha;
}

export function validateMigration(metadata, payload) {
  const result = z.object({ StatusCode: z.literal(200), FunctionError: z.never().optional() });
  if (
    !result.safeParse(metadata).success ||
    !z.object({ migrated: z.literal(true) }).safeParse(payload).success
  )
    throw new Error(
      'Migration did not succeed. Frontend publication stopped; inspect the migration function logs.',
    );
}

const outputsSchema = z.object({
  Stacks: z
    .array(
      z.object({
        Outputs: z.array(z.object({ OutputKey: z.string(), OutputValue: z.string() })).default([]),
      }),
    )
    .length(1),
});
function outputsFrom(value) {
  return Object.fromEntries(
    outputsSchema.parse(value).Stacks[0].Outputs.map((item) => [item.OutputKey, item.OutputValue]),
  );
}

export async function deployLambda(config, sha, { run = command, log = console.log } = {}) {
  const aws = (args, capture = true) =>
    run('aws', [...args, '--region', config.region, '--output', 'json', '--no-cli-pager'], {
      capture,
      env: { ...process.env, AWS_PAGER: '', AWS_CLI_AUTO_PROMPT: 'off' },
    });
  const json = (args) => JSON.parse(aws(args));
  const identity = z
    .object({ Account: z.string().regex(/^\d{12}$/) })
    .parse(json(['sts', 'get-caller-identity']));
  if (config.secretArn && identity.Account !== config.secretArn.split(':')[4])
    throw new Error('AWS credentials do not match the configured secret account.');
  const registry = `${identity.Account}.dkr.ecr.${config.region}.amazonaws.com`;
  const architecture = config.architecture ?? 'x86_64';
  const storageTag = config.storage === 'dynamodb' ? `${sha}-dynamodb` : sha;
  const imageTag = architecture === 'arm64' ? `${storageTag}-arm64` : storageTag;
  const image = `${registry}/${config.name}:${imageTag}`;
  let outputs = {};
  try {
    outputs = outputsFrom(json(['cloudformation', 'describe-stacks', '--stack-name', config.name]));
    if ((outputs.Storage ?? 'postgres') !== (config.storage ?? 'postgres'))
      throw new Error('Changing storage on an existing stack requires a data migration.');
    if (!outputs.FrontendUrl)
      throw new Error(
        'Existing stack is not an AppShell deployment (FrontendUrl output is missing).',
      );
  } catch (error) {
    if (!/ValidationError[\s\S]*does not exist/.test(error.message)) throw error;
  }
  log('Building the frontend before changing cloud resources…');
  run('npm', ['run', 'build'], { env: { ...process.env, VITE_API_URL: '' } });
  try {
    json(['ecr', 'describe-repositories', '--repository-names', config.name]);
  } catch (error) {
    if (!error.message.includes('RepositoryNotFoundException')) throw error;
    json([
      'ecr',
      'create-repository',
      '--repository-name',
      config.name,
      '--image-tag-mutability',
      'IMMUTABLE',
    ]);
  }
  try {
    json([
      'ecr',
      'describe-images',
      '--repository-name',
      config.name,
      '--image-ids',
      `imageTag=${imageTag}`,
    ]);
    log('Reusing the Lambda image already published for this commit.');
  } catch (error) {
    if (!error.message.includes('ImageNotFoundException')) throw error;
    const password = aws(['ecr', 'get-login-password']);
    run('docker', ['login', '--username', 'AWS', '--password-stdin', registry], {
      input: password + '\n',
      capture: true,
    });
    log('Building and publishing the Lambda image…');
    run('docker', [
      'buildx',
      'build',
      '--platform',
      architecture === 'arm64' ? 'linux/arm64' : 'linux/amd64',
      '--provenance=false',
      '--push',
      '--build-arg',
      `STORAGE=${config.storage ?? 'postgres'}`,
      '-f',
      'Dockerfile.lambda',
      '-t',
      image,
      '.',
    ]);
  }

  const apply = (origin) => {
    aws(
      [
        'cloudformation',
        'deploy',
        '--template-file',
        'deploy/lambda/template.yaml',
        '--stack-name',
        config.name,
        '--capabilities',
        'CAPABILITY_IAM',
        '--no-fail-on-empty-changeset',
        '--parameter-overrides',
        `ImageUri=${image}`,
        `Architecture=${architecture}`,
        `AppUrl=${origin}`,
        `SecretArn=${config.secretArn ?? ''}`,
        `Storage=${config.storage ?? 'postgres'}`,
        `MailFrom=${config.mailFrom}`,
        `MailBrand=${config.mailBrand}`,
        `MailSchedule=${config.mailSchedule}`,
        `Jobs=${config.jobs ?? 'disabled'}`,
      ],
      false,
    );
    return outputsFrom(json(['cloudformation', 'describe-stacks', '--stack-name', config.name]));
  };
  log('Applying the infrastructure template…');
  const firstDeploy = !outputs.FrontendUrl;
  outputs = apply(outputs.FrontendUrl ?? 'https://setup.invalid');
  if (!outputs.FrontendUrl) throw new Error('Stack did not return a frontend URL.');
  if (firstDeploy) {
    log('Configuring the generated frontend origin…');
    outputs = apply(outputs.FrontendUrl);
  }
  for (const key of ['MigrationFunction', 'FrontendBucketName', 'DistributionId', 'FrontendUrl'])
    if (!outputs[key]) throw new Error(`Stack is missing ${key}.`);

  const temporary = mkdtempSync(join(tmpdir(), 'appshell-migration-'));
  try {
    log('Running database migrations…');
    const responseFile = join(temporary, 'result.json');
    const metadata = json([
      'lambda',
      'invoke',
      '--function-name',
      outputs.MigrationFunction,
      '--invocation-type',
      'RequestResponse',
      '--cli-binary-format',
      'raw-in-base64-out',
      '--cli-read-timeout',
      '910',
      '--payload',
      '{}',
      responseFile,
    ]);
    validateMigration(metadata, JSON.parse(readFileSync(responseFile, 'utf8')));
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
  log('Publishing frontend assets, then index.html…');
  aws(
    [
      's3',
      'sync',
      'apps/web/dist',
      `s3://${outputs.FrontendBucketName}`,
      '--exclude',
      'index.html',
      '--cache-control',
      'public,max-age=60',
    ],
    false,
  );
  aws(
    [
      's3',
      'cp',
      'apps/web/dist/index.html',
      `s3://${outputs.FrontendBucketName}/index.html`,
      '--cache-control',
      'no-cache',
      '--content-type',
      'text/html',
    ],
    false,
  );
  const invalidation = z
    .object({ Invalidation: z.object({ Id: z.string() }) })
    .parse(
      json([
        'cloudfront',
        'create-invalidation',
        '--distribution-id',
        outputs.DistributionId,
        '--paths',
        '/*',
      ]),
    );
  aws(
    [
      'cloudfront',
      'wait',
      'invalidation-completed',
      '--distribution-id',
      outputs.DistributionId,
      '--id',
      invalidation.Invalidation.Id,
    ],
    false,
  );
  log(`Deployed ${sha} to ${outputs.FrontendUrl}`);
}

const deploySchema = z.object({ id: z.string().regex(/^dep-[a-z0-9]+$/), status: z.string() });
export async function deployDocker(
  config,
  sha,
  {
    token = process.env.RENDER_API_KEY,
    fetcher = fetch,
    sleep = setTimeout,
    log = console.log,
  } = {},
) {
  if (config.jobs === 'postgres') {
    await deployDocker({ ...config, jobs: 'disabled', serviceId: config.workerServiceId }, sha, {
      token,
      fetcher,
      sleep,
      log,
    });
  }
  const path = `https://api.render.com/v1/services/${config.serviceId}/deploys`;
  async function request(url, body) {
    const response = await fetcher(url, {
      method: body ? 'POST' : 'GET',
      headers: { authorization: `Bearer ${token}`, 'content-type': 'application/json' },
      ...(body ? { body: JSON.stringify(body) } : {}),
      signal: AbortSignal.timeout(30_000),
    });
    if (!response.ok)
      throw new Error(
        `Render returned HTTP ${response.status}. Check the service ID, credentials, and deployment dashboard.`,
      );
    return deploySchema.parse(await response.json());
  }
  let deployment = await request(path, { commitId: sha });
  log(`Render deployment ${deployment.id} started for ${sha}.`);
  for (let attempt = 0; attempt < 180; attempt++) {
    if (deployment.status === 'live') {
      log(`Deployed ${sha} to Render service ${config.serviceId}.`);
      return;
    }
    if (
      ![
        'created',
        'queued',
        'build_in_progress',
        'pre_deploy_in_progress',
        'update_in_progress',
      ].includes(deployment.status)
    )
      throw new Error(
        `Render deployment ${deployment.id}: ${deployment.status}. Check the deployment dashboard.`,
      );
    await sleep(10_000);
    deployment = await request(`${path}/${deployment.id}`);
  }
  throw new Error(
    `Timed out waiting for Render deployment ${deployment.id}. It may still be running; check Render before retrying.`,
  );
}
