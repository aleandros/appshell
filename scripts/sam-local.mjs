// Deploy the production SAM backend into LocalStack; the frontend runs through Vite.
import { command } from './deployment.mjs';
import { parseArgs } from 'node:util';
const { values } = parseArgs({
  options: { test: { type: 'boolean' }, architecture: { type: 'string' } },
});
// Match the Docker engine rather than the Node host (which may use a remote engine).
const dockerArchitecture = command('docker', ['info', '--format', '{{.Architecture}}'], {
  capture: true,
});
const architecture =
  values.architecture ?? (['aarch64', 'arm64'].includes(dockerArchitecture) ? 'arm64' : 'x86_64');
if (!['arm64', 'x86_64'].includes(architecture)) throw new Error('Use --architecture arm64|x86_64');
if (!process.env.LOCALSTACK_AUTH_TOKEN)
  throw new Error(
    'Set LOCALSTACK_AUTH_TOKEN for a plan with ECR and HTTP API support (currently Base or higher). Use compose.dynamodb.yaml for free development without an account.',
  );
const env = {
  ...process.env,
  AWS_ACCESS_KEY_ID: 'test',
  AWS_SECRET_ACCESS_KEY: 'test',
  AWS_SESSION_TOKEN: '',
  AWS_REGION: 'us-east-1',
  AWS_DEFAULT_REGION: 'us-east-1',
  AWS_EC2_METADATA_DISABLED: 'true',
};
const run = (name, args, options = {}) => command(name, args, { ...options, env });
const aws = (args) =>
  JSON.parse(
    run(
      'aws',
      ['--endpoint-url', 'http://localhost:4566', ...args, '--output', 'json', '--no-cli-pager'],
      { capture: true },
    ),
  );
run('docker', ['compose', '-f', 'compose.localstack.yaml', 'up', '-d', '--wait']);
const repository = 'appshell-local';
try {
  aws(['ecr', 'describe-repositories', '--repository-names', repository]);
} catch (error) {
  if (!error.message.includes('RepositoryNotFoundException')) throw error;
  aws(['ecr', 'create-repository', '--repository-name', repository]);
}
const image = `000000000000.dkr.ecr.us-east-1.localhost.localstack.cloud:4566/${repository}:local-${Date.now()}`;
run('docker', [
  'buildx',
  'build',
  '--platform',
  architecture === 'arm64' ? 'linux/arm64' : 'linux/amd64',
  '--provenance=false',
  '--load',
  '--build-arg',
  'STORAGE=dynamodb',
  '-f',
  'Dockerfile.lambda',
  '-t',
  image,
  '.',
]);
run('docker', ['push', image]);
run('lstk', [
  'sam',
  'deploy',
  '--template-file',
  'deploy/lambda/template.yaml',
  '--stack-name',
  'appshell-local',
  '--capabilities',
  'CAPABILITY_IAM',
  '--no-confirm-changeset',
  '--no-fail-on-empty-changeset',
  '--parameter-overrides',
  `ImageUri=${image}`,
  `Architecture=${architecture}`,
  'Storage=dynamodb',
  'Jobs=sqs',
  'LocalDevelopment=true',
  'DynamoEndpoint=http://localstack:4566',
  'AppUrl=http://localhost:5173',
  'MailFrom=hello@appshell.test',
  'MailBrand=AppShell',
]);
const stack = aws(['cloudformation', 'describe-stacks', '--stack-name', 'appshell-local']);
const outputs = Object.fromEntries(
  stack.Stacks[0].Outputs.map(({ OutputKey, OutputValue }) => [OutputKey, OutputValue]),
);
const apiId = outputs.ApiUrl.split('://')[1].split('.')[0];
const endpoint = `http://${apiId}.execute-api.localhost.localstack.cloud:4566`;
console.log(
  `Local SAM API: ${endpoint}\nRun API_PROXY_TARGET=${endpoint} API_TEST_URL=${endpoint} npm run test:e2e\nRun API_TEST_URL=${endpoint} npm run test:mail\nStop with docker compose -f compose.localstack.yaml down`,
);

if (values.test) {
  const testEnv = { ...env, CI: 'true', API_PROXY_TARGET: endpoint, API_TEST_URL: endpoint };
  command('npm', ['run', 'test:e2e'], { env: testEnv });
  command('npm', ['run', 'test:mail'], { env: testEnv });
}
