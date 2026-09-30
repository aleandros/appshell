import assert from 'node:assert/strict';
import { test } from 'node:test';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { deploymentSchema, missingSettings } from './deployment-config.mjs';
import { deployDocker, deployLambda, releaseCommit, validateMigration } from './deployment.mjs';

const sha = 'a'.repeat(40);
const lambda = {
  version: 1,
  name: 'test-app',
  mode: 'lambda',
  jobs: 'disabled',
  autoDeploy: false,
  region: 'us-east-1',
  secretArn: 'arn:aws:secretsmanager:us-east-1:123456789012:secret:appshell-test',
  roleArn: 'arn:aws:iam::123456789012:role/deploy',
  mailFrom: 'My App <hello@example.com>',
  mailBrand: 'My App',
  mailSchedule: 'rate(1 minute)',
};
const docker = {
  version: 1,
  name: 'test-app',
  mode: 'docker',
  jobs: 'disabled',
  autoDeploy: false,
  serviceId: 'srv-example',
};
const log = () => {};

function fixture(t) {
  const directory = mkdtempSync(join(tmpdir(), 'appshell-deployment-test-'));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  return directory;
}
function script(name, cwd, args = [], env = {}) {
  return spawnSync(process.execPath, [fileURLToPath(new URL(name, import.meta.url)), ...args], {
    cwd,
    encoding: 'utf8',
    env: { ...process.env, ...env },
  });
}

test('deployment config rejects secrets, mismatched accounts/regions, and unknown modes', () => {
  assert.deepEqual(deploymentSchema.parse(lambda), lambda);
  for (const value of [
    { ...docker, apiKey: 'do-not-save-me' },
    { ...lambda, region: 'eu-west-1' },
    { ...lambda, roleArn: 'arn:aws:iam::999999999999:role/deploy' },
    { ...docker, mode: 'dynamo' },
    { ...docker, name: '../other-app' },
    { ...docker, serviceId: 'srv-example\nmode=lambda' },
    { ...docker, jobs: 'sqs' },
    { ...lambda, jobs: 'postgres' },
    { ...docker, jobs: 'postgres', workerServiceId: docker.serviceId },
  ])
    assert.equal(deploymentSchema.safeParse(value).success, false);
  assert.equal(missingSettings(docker, {}).length, 1);
  assert.equal(missingSettings(lambda, {}).length, 0);
  assert.equal(deploymentSchema.parse({ ...docker, jobs: undefined }).jobs, 'disabled');
  assert.deepEqual(missingSettings({ ...docker, jobs: 'postgres' }, { RENDER_API_KEY: 'test' }), [
    'workerServiceId (separate Render worker)',
  ]);
});

test('setup can save/update/switch modes without cloud tools; plans never invoke them', (t) => {
  const cwd = fixture(t);
  let result = script(
    'setup.mjs',
    cwd,
    ['--mode', 'docker', '--name', 'new-app', '--service-id', 'srv-example', '--auto-deploy'],
    { PATH: '' },
  );
  assert.equal(result.status, 0, result.stderr);
  const file = join(cwd, 'appshell.deploy.json');
  assert.equal(JSON.parse(readFileSync(file)).autoDeploy, true);
  result = script('setup.mjs', cwd, ['--mode', 'docker', '--no-auto-deploy'], { PATH: '' });
  assert.equal(result.status, 0, result.stderr);
  assert.equal(JSON.parse(readFileSync(file)).serviceId, 'srv-example');
  result = script(
    'setup.mjs',
    cwd,
    ['--mode', 'docker', '--jobs', 'postgres', '--worker-service-id', 'srv-worker'],
    { PATH: '' },
  );
  assert.equal(result.status, 0, result.stderr);
  assert.equal(JSON.parse(readFileSync(file)).workerServiceId, 'srv-worker');
  result = script('setup.mjs', cwd, ['--mode', 'lambda'], { PATH: '' });
  assert.equal(result.status, 0, result.stderr);
  const config = JSON.parse(readFileSync(file));
  assert.equal(config.serviceId, undefined);
  assert.equal(config.name, 'new-app');
  assert.equal(config.autoDeploy, false);
  assert.equal(config.jobs, 'disabled');
  assert.equal(config.workerServiceId, undefined);
  const before = readFileSync(file, 'utf8');
  result = script('deploy.mjs', cwd, ['--dry-run'], { PATH: '' });
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /Still needed/);
  assert.match(result.stdout, /No cloud calls were made/);
  assert.equal(readFileSync(file, 'utf8'), before);
  result = script('setup.mjs', cwd, ['--mode', 'wrong'], { PATH: '' });
  assert.notEqual(result.status, 0);
  assert.equal(readFileSync(file, 'utf8'), before);
});

test('GitHub settings skip unconfigured/disabled apps and require OIDC for Lambda', (t) => {
  const cwd = fixture(t);
  const output = join(cwd, 'output');
  writeFileSync(output, '');
  const env = { GITHUB_OUTPUT: output, APPSHELL_MANUAL_DEPLOY: 'false' };
  assert.equal(script('deployment-workflow.mjs', cwd, [], env).status, 0);
  writeFileSync(join(cwd, 'appshell.deploy.json'), JSON.stringify(docker));
  assert.equal(script('deployment-workflow.mjs', cwd, [], env).status, 0);
  assert.equal(readFileSync(output, 'utf8'), '');
  assert.equal(
    script('deployment-workflow.mjs', cwd, [], { ...env, APPSHELL_MANUAL_DEPLOY: 'true' }).status,
    0,
  );
  assert.match(readFileSync(output, 'utf8'), /mode=docker/);
  writeFileSync(
    join(cwd, 'appshell.deploy.json'),
    JSON.stringify({ ...lambda, roleArn: undefined, autoDeploy: true }),
  );
  assert.notEqual(script('deployment-workflow.mjs', cwd, [], env).status, 0);
});

test('deployments require a clean commit; migration CLI success is insufficient', () => {
  assert.throws(() => releaseCommit(() => ' M file'), /working-tree/);
  assert.equal(
    releaseCommit((_, args) => (args[0] === 'status' ? '' : sha)),
    sha,
  );
  for (const [meta, payload] of [
    [{ StatusCode: 200, FunctionError: 'Unhandled' }, { migrated: true }],
    [{ StatusCode: 200 }, {}],
    [{ StatusCode: 202 }, { migrated: true }],
  ])
    assert.throws(() => validateMigration(meta, payload), /Frontend publication stopped/);
  validateMigration({ StatusCode: 200 }, { migrated: true });
});

function awsFake({
  migrationError = false,
  permissionError = false,
  existing = false,
  account = '123456789012',
} = {}) {
  const calls = [];
  let applied = existing;
  const run = (cmd, args, options) => {
    calls.push({ cmd, args, options });
    if (cmd !== 'aws') return '';
    const operation = args.slice(0, 2).join(' ');
    if (operation === 'sts get-caller-identity') return JSON.stringify({ Account: account });
    if (operation === 'cloudformation describe-stacks') {
      if (!applied) throw new Error('ValidationError: Stack does not exist');
      return JSON.stringify({
        Stacks: [
          {
            Outputs: Object.entries({
              FrontendUrl: 'https://example.cloudfront.net',
              MigrationFunction: 'migration',
              FrontendBucketName: 'frontend',
              DistributionId: 'D123',
            }).map(([OutputKey, OutputValue]) => ({ OutputKey, OutputValue })),
          },
        ],
      });
    }
    if (operation === 'cloudformation deploy') {
      applied = true;
      return '';
    }
    if (operation === 'ecr describe-repositories') {
      if (permissionError) throw new Error('AccessDeniedException');
      if (!existing) throw new Error('RepositoryNotFoundException');
    }
    if (operation === 'ecr describe-images' && !existing) throw new Error('ImageNotFoundException');
    if (operation === 'ecr get-login-password') return 'ephemeral-token';
    if (operation === 'lambda invoke') {
      writeFileSync(
        args[args.indexOf('--region') - 1],
        JSON.stringify(migrationError ? {} : { migrated: true }),
      );
      return JSON.stringify({
        StatusCode: 200,
        ...(migrationError ? { FunctionError: 'Unhandled' } : {}),
      });
    }
    if (operation === 'cloudfront create-invalidation')
      return JSON.stringify({ Invalidation: { Id: 'I123' } });
    return '{}';
  };
  return { run, calls };
}

test('Lambda first deploy configures origin and gates index publication on successful migration', async () => {
  const fake = awsFake();
  await deployLambda(lambda, sha, { ...fake, log });
  const calls = fake.calls;
  const apply = calls.filter((c) => c.args[0] === 'cloudformation' && c.args[1] === 'deploy');
  assert.equal(apply.length, 2);
  assert.ok(apply[0].args.includes('AppUrl=https://setup.invalid'));
  assert.ok(apply[1].args.includes('AppUrl=https://example.cloudfront.net'));
  assert.ok(apply[0].args.includes('MailFrom=My App <hello@example.com>'));
  assert.ok(apply[0].args.includes('Jobs=disabled'));
  assert.equal(calls.find((c) => c.cmd === 'npm').options.env.VITE_API_URL, '');
  const login = calls.find((c) => c.cmd === 'docker' && c.args[0] === 'login');
  assert.equal(login.options.input, 'ephemeral-token\n');
  assert.ok(!login.args.includes('ephemeral-token'));
  const migrate = calls.findIndex((c) => c.args[0] === 'lambda');
  const assets = calls.findIndex((c) => c.args[0] === 's3' && c.args[1] === 'sync');
  const index = calls.findIndex((c) => c.args[0] === 's3' && c.args[1] === 'cp');
  assert.ok(migrate < assets && assets < index);
  assert.ok(!calls[assets].args.includes('--delete'));
});

test('Lambda retries reuse images and update the discovered origin', async () => {
  const fake = awsFake({ existing: true });
  await deployLambda({ ...lambda, jobs: 'sqs' }, sha, { ...fake, log });
  assert.equal(fake.calls.filter((c) => c.cmd === 'docker').length, 0);
  const updates = fake.calls.filter(
    (c) => c.args[0] === 'cloudformation' && c.args[1] === 'deploy',
  );
  assert.equal(updates.length, 1);
  assert.ok(updates[0].args.includes('AppUrl=https://example.cloudfront.net'));
  assert.ok(updates[0].args.includes('Jobs=sqs'));
});

test('Lambda stops on migration failure, permission errors, or wrong AWS account', async () => {
  for (const options of [
    { migrationError: true },
    { permissionError: true },
    { account: '999999999999' },
  ]) {
    const fake = awsFake(options);
    await assert.rejects(deployLambda(lambda, sha, { ...fake, log }));
    assert.ok(!fake.calls.some((c) => c.args[0] === 's3'));
    if (!options.migrationError)
      assert.ok(
        !fake.calls.some((c) => c.args[1] === 'create-repository' || c.args[1] === 'deploy'),
      );
  }
});

test('Render pins the Git commit and waits for live; provider failures stop the release', async () => {
  for (const status of ['live', 'build_failed']) {
    const requests = [];
    const fetcher = async (url, options) => {
      requests.push({ url, options });
      return {
        ok: true,
        json: async () => ({
          id: 'dep-example',
          status: requests.length === 1 ? 'build_in_progress' : status,
        }),
      };
    };
    const promise = deployDocker(docker, sha, {
      token: 'private-token',
      fetcher,
      sleep: async () => {},
      log,
    });
    if (status === 'live') await promise;
    else await assert.rejects(promise, /build_failed/);
    assert.deepEqual(JSON.parse(requests[0].options.body), { commitId: sha });
    assert.equal(requests[0].options.headers.authorization, 'Bearer private-token');
    assert.ok(requests[1].url.endsWith('/dep-example'));
  }
});

test('Postgres jobs deploy the worker before the API and stop if the worker fails', async () => {
  for (const workerStatus of ['live', 'build_failed']) {
    const requests = [];
    const fetcher = async (url, options) => {
      requests.push({ url, options });
      return {
        ok: true,
        json: async () => ({
          id: 'dep-example',
          status: url.includes('srv-worker') ? workerStatus : 'live',
        }),
      };
    };
    const promise = deployDocker(
      { ...docker, jobs: 'postgres', workerServiceId: 'srv-worker' },
      sha,
      { token: 'test', fetcher, log },
    );
    if (workerStatus === 'live') {
      await promise;
      assert.equal(requests.length, 2);
      assert.match(requests[1].url, /srv-example\/deploys$/);
    } else {
      await assert.rejects(promise, /build_failed/);
      assert.equal(requests.length, 1);
    }
    assert.match(requests[0].url, /srv-worker\/deploys$/);
    assert.deepEqual(JSON.parse(requests[0].options.body), { commitId: sha });
  }
});

test('Lambda refuses to overwrite unrelated stacks and stops if image publishing fails', async () => {
  for (const failure of ['stack', 'image']) {
    const fake = awsFake();
    const run = (cmd, args, options) => {
      if (failure === 'stack' && args[0] === 'cloudformation' && args[1] === 'describe-stacks')
        return JSON.stringify({ Stacks: [{}] });
      if (failure === 'image' && cmd === 'docker' && args[0] === 'buildx')
        throw new Error('image publication failed');
      return fake.run(cmd, args, options);
    };
    await assert.rejects(
      deployLambda(lambda, sha, { run, log }),
      failure === 'stack' ? /not an AppShell/ : /publication failed/,
    );
    assert.ok(!fake.calls.some((c) => c.args[0] === 'cloudformation' && c.args[1] === 'deploy'));
  }
});
