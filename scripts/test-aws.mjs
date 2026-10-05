// Opt-in cloud smoke test. Uses a fresh stack and removes only its own resources.
// Real SES delivery is deliberately excluded; local Mailpit tests cover mail flows.
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { setTimeout } from 'node:timers/promises';
import { parseArgs } from 'node:util';
import { command, deployLambda } from './deployment.mjs';

const { values } = parseArgs({
  options: { region: { type: 'string', default: 'us-east-1' }, architecture: { type: 'string' } },
});
const region = values.region;
const engine = command('docker', ['info', '--format', '{{.Architecture}}'], { capture: true });
const architecture =
  values.architecture ?? (['arm64', 'aarch64'].includes(engine) ? 'arm64' : 'x86_64');
assert.ok(['arm64', 'x86_64'].includes(architecture), 'Use --architecture arm64|x86_64');
const name = `appshell-smoke-${Date.now()}-${randomUUID().slice(0, 8)}`;
const directory = mkdtempSync(join(tmpdir(), 'appshell-aws-smoke-'));
const report = { name, region, architecture, checks: [], resources: [], cleaned: false };
const save = () =>
  writeFileSync(join(directory, 'report.json'), JSON.stringify(report, null, 2) + '\n');
const aws = (args, capture = true) =>
  command('aws', [...args, '--region', region, '--output', 'json', '--no-cli-pager'], { capture });
const json = (args) => JSON.parse(aws(args));
const check = (message) => {
  report.checks.push(message);
  save();
  console.log(message);
};
const identity = json(['sts', 'get-caller-identity']);
report.account = identity.Account;
save();
console.log(
  `Temporary AWS test: ${name}, ${region}, ${architecture}, account ${identity.Account}.\nReport: ${directory}/report.json`,
);

async function cleanup() {
  console.log(`Removing temporary AWS resources for ${name}…`);
  let stack;
  try {
    stack = json(['cloudformation', 'describe-stacks', '--stack-name', name]).Stacks[0];
  } catch (error) {
    if (!/ValidationError[\s\S]*does not exist/.test(error.message)) throw error;
  }
  if (stack) {
    report.resources = json([
      'cloudformation',
      'list-stack-resources',
      '--stack-name',
      stack.StackId,
    ]).StackResourceSummaries;
    report.stackId = stack.StackId;
    save();
    aws(['cloudformation', 'delete-stack', '--stack-name', stack.StackId]);
    aws(['cloudformation', 'wait', 'stack-delete-complete', '--stack-name', stack.StackId], false);
    // Production deliberately retains data. Delete those resources only for this
    // fresh smoke stack, using exact physical IDs captured before stack deletion.
    for (const resource of report.resources) {
      const id = resource.PhysicalResourceId;
      if (!id) continue;
      if (resource.ResourceType === 'AWS::DynamoDB::Table')
        aws(['dynamodb', 'delete-table', '--table-name', id]);
      if (resource.ResourceType === 'AWS::SQS::Queue')
        aws(['sqs', 'delete-queue', '--queue-url', id]);
      if (resource.ResourceType === 'AWS::S3::Bucket') {
        aws(['s3', 'rm', `s3://${id}`, '--recursive'], false);
        aws(['s3api', 'delete-bucket', '--bucket', id]);
      }
      if (resource.ResourceType === 'AWS::Lambda::Function') {
        try {
          aws(['logs', 'delete-log-group', '--log-group-name', `/aws/lambda/${id}`]);
        } catch (error) {
          if (!error.message.includes('ResourceNotFoundException')) throw error;
        }
      }
    }
  }
  try {
    aws(['ecr', 'delete-repository', '--repository-name', name, '--force']);
  } catch (error) {
    if (!error.message.includes('RepositoryNotFoundException')) throw error;
  }
  report.cleaned = true;
  save();
  console.log(
    'Temporary stack, retained test data, queues, frontend assets, logs and ECR image removed.',
  );
}

let failure;
try {
  await deployLambda(
    {
      version: 1,
      mode: 'lambda',
      storage: 'dynamodb',
      jobs: 'sqs',
      architecture,
      name,
      region,
      mailFrom: 'smoke@example.invalid',
      mailBrand: 'AppShell',
      mailSchedule: 'rate(15 minutes)',
    },
    name,
  );
  const stack = json(['cloudformation', 'describe-stacks', '--stack-name', name]).Stacks[0];
  const outputs = Object.fromEntries(
    stack.Outputs.map(({ OutputKey, OutputValue }) => [OutputKey, OutputValue]),
  );
  report.outputs = outputs;
  check('SAM deployment and schema-validation Lambda passed.');
  const ready = await fetch(`${outputs.ApiUrl}/ready`, { signal: AbortSignal.timeout(30_000) });
  assert.equal(ready.status, 200);
  check('API Gateway → Lambda → DynamoDB readiness passed.');
  command('npm', ['run', 'test:e2e'], {
    env: {
      ...process.env,
      CI: 'true',
      E2E_BASE_URL: outputs.FrontendUrl,
      API_TEST_URL: outputs.ApiUrl,
      E2E_EMAIL_DOMAIN: 'example.invalid',
    },
  });
  check(
    'CloudFront browser tests passed (desktop/mobile, secure sessions, signup and logout). Admin UI fixtures remain mocked.',
  );

  // No GSI listing and no API request: only the stream can publish this job.
  // Its mail is already delivered, testing idempotent execution without SES.
  const mailId = randomUUID();
  const jobId = randomUUID();
  const timestamp = new Date().toISOString();
  function put(kind, id, data) {
    aws([
      'dynamodb',
      'put-item',
      '--table-name',
      outputs.DataTableName,
      '--condition-expression',
      'attribute_not_exists(pk)',
      '--item',
      JSON.stringify({
        pk: { S: `${kind}#${id}` },
        sk: { S: 'record' },
        version: { N: '1' },
        data: { S: JSON.stringify({ id, created_at: timestamp, deleted_at: null, ...data }) },
      }),
    ]);
  }
  put('mail', mailId, { sent_at: timestamp, body: '[delivered]' });
  put('job', jobId, {
    payload: { type: 'mail.v1', mail_id: mailId },
    attempts: 0,
    lease_version: 0,
    available_at: timestamp,
  });
  const deadline = Date.now() + 180_000;
  let completed;
  while (Date.now() < deadline) {
    const item = json([
      'dynamodb',
      'get-item',
      '--table-name',
      outputs.DataTableName,
      '--consistent-read',
      '--key',
      JSON.stringify({ pk: { S: `job#${jobId}` }, sk: { S: 'record' } }),
    ]);
    const data = JSON.parse(item.Item.data.S);
    if (data.completed_at) {
      completed = data;
      break;
    }
    await setTimeout(3000);
  }
  assert.ok(completed, 'Stream → dispatcher → SQS → worker must complete the redelivery fixture');
  assert.equal(completed.attempts, 1);
  assert.deepEqual(completed.payload, {});
  check(
    'DynamoDB Streams → dispatcher → SQS → worker → atomic completion passed; delivered-mail replay avoided SES.',
  );
  report.passed = true;
} catch (error) {
  report.error = error.message;
  failure = error;
} finally {
  save();
  try {
    await cleanup();
  } catch (error) {
    report.cleanupError = error.message;
    save();
    failure = new AggregateError(
      [failure, error].filter(Boolean),
      `Cleanup incomplete: ${directory}/report.json`,
    );
  }
}

if (failure) throw failure;
