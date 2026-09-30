import { spawnSync } from 'node:child_process';

// Source archives, CI, and image builds do not need local Git hooks.
if (process.env.CI || process.env.APPSHELL_SKIP_HOOKS === '1') process.exit(0);
const git = (...args) => spawnSync('git', args, { encoding: 'utf8' });
if (git('rev-parse', '--show-toplevel').status !== 0) process.exit(0);
const current = git('config', '--get', 'core.hooksPath').stdout.trim();
if (current && current !== '.githooks') {
  console.warn(`Keeping existing core.hooksPath=${current}. See README for hook integration.`);
  process.exit(0);
}
const result = git('config', '--local', 'core.hooksPath', '.githooks');
if (result.status !== 0) {
  console.error(result.stderr || result.error?.message || 'Could not install Git hooks');
  process.exit(1);
}
