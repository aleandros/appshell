import assert from 'node:assert/strict';
import { test } from 'node:test';
import { importViolation, inspectSource, cycles } from './architecture.mjs';

test('rejects forbidden dependency directions and private feature imports', () => {
  for (const [from, to] of [
    ['pages/auth.tsx', 'lib/api.ts'],
    ['pages/auth.tsx', 'features/identity/api.ts'],
    ['features/billing/api.ts', 'features/identity/index.ts'],
    ['components/button.tsx', 'pages/auth.tsx'],
    ['components/button.tsx', 'features/identity/index.ts'],
    ['lib/errors.ts', 'features/identity/index.ts'],
    ['features/identity/api.ts', 'app/workspace.tsx'],
    ['pages/auth.tsx', 'lib/api.generated.ts'],
    ['other.ts', 'lib/errors.ts'],
  ])
    assert.ok(importViolation(from, to), `${from} -> ${to}`);
  assert.equal(importViolation('pages/auth.tsx', 'features/identity/index.ts'), undefined);
  assert.equal(importViolation('features/identity/api.ts', 'lib/api.ts'), undefined);
});

test('inspects re-exports, dynamic imports, type imports and network aliases', () => {
  const result = inspectSource(
    'pages/test.tsx',
    `
    export * from '../features/identity/api';
    const lazy = import('../lib/api');
    type Private = import('../features/billing/api').Private;
    const transport = window.fetch;
  `,
  );
  assert.equal(result.imports.length, 3);
  assert.ok(result.errors.length);
  assert.ok(inspectSource('pages/test.tsx', 'const api = globalThis["fetch"];').errors.length);
  assert.ok(inspectSource('pages/test.tsx', 'import(variable);').errors.length);
  assert.equal(
    inspectSource('pages/test.tsx', '// fetch()\nconst text = "fetch";').errors.length,
    0,
  );
  assert.equal(inspectSource('lib/api.ts', 'fetch(url);').errors.length, 0);
});

test('rejects cycles including type-only dependency cycles', () => {
  assert.equal(
    cycles(
      new Map([
        ['a', ['b']],
        ['b', ['a']],
      ]),
    ).length,
    1,
  );
  assert.equal(
    cycles(
      new Map([
        ['a', ['b']],
        ['b', []],
      ]),
    ).length,
    0,
  );
});
