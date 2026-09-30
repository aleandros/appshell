import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

// Generate outside the working tree: checks must not rewrite tracked contracts.
const temporary = mkdtempSync(join(tmpdir(), 'appshell-contract-'));
function run(command, args) {
  const result = spawnSync(command, args, { encoding: 'utf8', maxBuffer: 10 * 1024 * 1024 });
  if (result.status !== 0) {
    throw new Error(result.stderr || result.error?.message || `${command} failed`);
  }
  return result.stdout;
}
try {
  const schema = run(process.execPath, [
    'scripts/rust.mjs',
    'run',
    '--quiet',
    '--locked',
    '--bin',
    'export-openapi',
  ]);
  const input = join(temporary, 'openapi.json');
  const output = join(temporary, 'api.generated.ts');
  writeFileSync(input, schema);
  run(process.execPath, ['node_modules/openapi-typescript/bin/cli.js', input, '-o', output]);
  for (const [file, generated] of [
    ['openapi.json', schema],
    ['apps/web/src/lib/api.generated.ts', readFileSync(output, 'utf8')],
  ]) {
    if (readFileSync(file, 'utf8') !== generated) {
      throw new Error(`${file} is stale. Run npm run generate:api and commit both contracts.`);
    }
  }
} finally {
  rmSync(temporary, { recursive: true, force: true });
}
