import assert from 'node:assert/strict';
const origin = process.argv[2] ?? 'http://localhost:8081';
for (const path of ['/health', '/ready', '/', '/app/settings']) {
  const response = await fetch(`${origin}${path}`);
  assert.equal(response.status, 200, path);
  if (path === '/' || path === '/app/settings')
    assert.match(await response.text(), /<div id="root">/);
}
const missing = await fetch(`${origin}/api/missing`);
assert.equal(missing.status, 404);
assert.equal((await missing.json()).code, 'not_found');
const session = await fetch(`${origin}/api/auth/session`);
assert.equal(session.status, 401);
const csrf = await fetch(`${origin}/api/auth/logout`, {
  method: 'POST',
  headers: { origin: 'https://untrusted.example' },
});
assert.equal(csrf.status, 403);
console.log(
  'Container smoke check passed: static assets, SPA fallback, readiness, API errors, and CSRF.',
);
