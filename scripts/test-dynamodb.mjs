import { spawnSync } from 'node:child_process';
const env = {
  ...process.env,
  AWS_ACCESS_KEY_ID: 'local',
  AWS_SECRET_ACCESS_KEY: 'local',
  AWS_REGION: 'us-east-1',
  AWS_EC2_METADATA_DISABLED: 'true',
  DYNAMODB_TABLE: 'appshell-test',
  DYNAMODB_ENDPOINT_URL: 'http://localhost:8000',
  DYNAMODB_CREATE_TABLE: 'true',
  APP_ENV: 'development',
  MAIL_MODE: 'smtp',
};
const testArgs = [
  'test',
  '--locked',
  '-p',
  'appshell-api',
  '--no-default-features',
  '--features',
  'dynamodb,lambda,ses',
  '--lib',
];
const preference = process.env.APPSHELL_RUST_RUNNER;
if (preference && !['local', 'docker'].includes(preference))
  throw new Error('APPSHELL_RUST_RUNNER must be local or docker');
const native =
  preference === 'local' ||
  (!preference && spawnSync('cargo', ['--version'], { stdio: 'ignore' }).status === 0);
for (const [command, args] of [
  ['docker', ['compose', '-f', 'compose.dynamodb.yaml', 'up', '-d', '--wait', 'dynamodb']],
  native
    ? ['cargo', testArgs]
    : [
        'docker',
        [
          'compose',
          '-f',
          'compose.dynamodb.yaml',
          'run',
          '--rm',
          '--no-deps',
          'api',
          'cargo',
          ...testArgs,
        ],
      ],
]) {
  const result = spawnSync(command, args, { env, stdio: 'inherit' });
  if (result.status !== 0) process.exit(result.status ?? 1);
}
