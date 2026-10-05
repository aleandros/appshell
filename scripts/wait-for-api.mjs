// Browser tests must start against a ready API, including after a Compose restart.
import { setTimeout } from 'node:timers/promises';
const deadline = Date.now() + 60_000;
let ready = false;
while (Date.now() < deadline) {
  try {
    const response = await fetch(`${process.env.API_TEST_URL ?? 'http://localhost:8080'}/ready`, {
      signal: AbortSignal.timeout(2_000),
    });
    if (response.ok) {
      ready = true;
      break;
    }
  } catch {
    // A refused/reset connection is expected while Cargo recompiles the API.
  }
  await setTimeout(500);
}
if (!ready)
  throw new Error(
    'API did not become ready within 60 seconds. Start the selected backend or set API_TEST_URL.',
  );
