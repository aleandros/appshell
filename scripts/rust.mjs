import { spawnSync } from 'node:child_process';

// Use the same commands locally and in CI; Docker supports machines without Rust.
const args = process.argv.slice(2);
if (!args.length) throw new Error('Expected a Cargo command');
const preference = process.env.APPSHELL_RUST_RUNNER;
if (preference && !['local', 'docker'].includes(preference))
  throw new Error('APPSHELL_RUST_RUNNER must be local or docker');
const native =
  preference === 'local' ||
  (!preference && spawnSync('cargo', ['--version'], { stdio: 'ignore' }).status === 0);
const databaseTests =
  args[0] === 'test' && !args.includes('--test') && !args.includes('--lib') && !args.includes('-p');
if (native && databaseTests && !process.env.TEST_DATABASE_URL)
  throw new Error(
    'Set TEST_DATABASE_URL to a disposable test database, or use APPSHELL_RUST_RUNNER=docker. Integration tests must not silently skip.',
  );
const dockerArgs = ['compose', 'run', '--rm', '--no-deps'];
if (process.env.TEST_DATABASE_URL || databaseTests)
  dockerArgs.push(
    '-e',
    `TEST_DATABASE_URL=${process.env.TEST_DATABASE_URL ?? 'postgres://appshell:appshell@db:5432/appshell'}`,
  );
const result = spawnSync(
  native ? 'cargo' : 'docker',
  native ? args : [...dockerArgs, 'api', 'cargo', ...args],
  { stdio: 'inherit' },
);
if (result.error) console.error(result.error.message);
process.exitCode = result.status ?? 1;
