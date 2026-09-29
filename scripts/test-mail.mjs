// Exercise the real API -> transactional outbox -> SMTP -> Mailpit pipeline.
// Unique .test addresses avoid touching other local messages or accounts.
import assert from 'node:assert/strict';
import { setTimeout } from 'node:timers/promises';
import './wait-for-api.mjs';
const api = 'http://localhost:8080/api';
const inbox = 'http://localhost:8025';
const origin = 'http://localhost:5173';
const email = `mail-${crypto.randomUUID()}@example.test`;
const newEmail = `changed-${crypto.randomUUID()}@example.test`;
const inviteEmail = `invite-${crypto.randomUUID()}@example.test`;
const password = 'correct horse battery staple';
async function post(path, body, cookie) {
  const response = await fetch(`${api}${path}`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Origin: origin,
      ...(cookie ? { Cookie: cookie } : {}),
    },
    body: JSON.stringify(body),
    signal: AbortSignal.timeout(10_000),
  });
  const result = await response.json();
  assert.ok(response.ok, `${path}: ${response.status} ${JSON.stringify(result)}`);
  return { result, cookie: response.headers.get('set-cookie')?.split(';')[0] };
}
async function message(recipient, subject) {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    const response = await fetch(
      `${inbox}/api/v1/search?query=${encodeURIComponent(`to:${recipient}`)}`,
      { signal: AbortSignal.timeout(2000) },
    );
    assert.ok(response.ok, 'Mailpit must be running');
    const data = await response.json();
    const match = data.messages.find((item) => item.Subject === subject);
    if (match) {
      const full = await fetch(`${inbox}/api/v1/message/${match.ID}`).then((r) => r.json());
      assert.ok(full.HTML.includes('AppShell'), 'branded HTML is present');
      assert.ok(full.HTML.includes('role="presentation"'), 'shared template is present');
      assert.ok(full.HTML.includes('style='), 'styles are inlined for mail clients');
      assert.ok(full.Text.includes(subject), 'plain text alternative is present');
      assert.ok(full.HTML.includes('href='), 'action is clickable');
      assert.ok(!full.HTML.includes('<script>'), 'untrusted content is escaped');
      return full;
    }
    await setTimeout(500);
  }
  throw new Error(`Missing ${subject} in Mailpit. Use MAIL_MODE=smtp and run the mail worker.`);
}
function token(mail) {
  const value = mail.Text.match(/#token=([a-f0-9]{64})/)?.[1];
  assert.ok(value, 'action token is present');
  assert.ok(mail.HTML.includes(value), 'HTML and text use the same token');
  return value;
}
const signup = await post('/auth/signup', {
  email,
  password,
  name: 'Mail <script>test</script>',
  organization: 'Mail preview',
});
await post('/auth/verify-email', { token: token(await message(email, 'Verify your email')) });
const org = signup.result.organizations[0].id;
await post(
  `/organizations/${org}/invitations`,
  { email: inviteEmail, role: 'member' },
  signup.cookie,
);
const invite = await message(inviteEmail, "You're invited to a workspace");
assert.ok(invite.HTML.includes('&lt;script&gt;test&lt;/script&gt;'), 'inviter name is escaped');
await post('/auth/change-email', { email: newEmail, password }, signup.cookie);
await message(email, 'Email change requested');
await post('/auth/confirm-email', {
  token: token(await message(newEmail, 'Confirm your new email')),
});
await post('/auth/forgot-password', { email: newEmail });
await post('/auth/reset-password', {
  token: token(await message(newEmail, 'Reset your password')),
  password: 'a different sufficiently long password',
});
await post('/auth/login', { email: newEmail, password: 'a different sufficiently long password' });
console.log(
  'Mailpit delivery passed: verification, invitation, email-change notice, confirmation, and password reset (HTML + text).',
);
