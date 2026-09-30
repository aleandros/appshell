import { appendFileSync, existsSync } from 'node:fs';
import { configPath, readConfig } from './deployment-config.mjs';

// Only emits schema-validated, non-secret settings to the reusable workflow.
if (existsSync(configPath)) {
  const config = readConfig();
  if (process.env.APPSHELL_MANUAL_DEPLOY === 'true' || config.autoDeploy) {
    if (config.mode === 'lambda' && !config.roleArn)
      throw new Error('Configure roleArn with npm run setup before GitHub deployment.');
    appendFileSync(
      process.env.GITHUB_OUTPUT,
      `mode=${config.mode}\nregion=${config.mode === 'lambda' ? config.region : ''}\nrole=${config.mode === 'lambda' ? config.roleArn : ''}\n`,
    );
  }
} else if (process.env.APPSHELL_MANUAL_DEPLOY === 'true') {
  throw new Error('Commit appshell.deploy.json from npm run setup before deploying.');
}
