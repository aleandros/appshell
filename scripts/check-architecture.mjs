import { check } from './architecture.mjs';
const errors = check();
if (errors.length) {
  console.error(errors.join('\n'));
  process.exitCode = 1;
} else {
  console.log('Frontend architectural boundaries passed.');
}
