import { parseArgs } from 'node:util';
import { readConfig, deploymentPlan, missingSettings } from './deployment-config.mjs';
import { deployDocker, deployLambda, releaseCommit } from './deployment.mjs';

const { values } = parseArgs({ options: { 'dry-run': { type: 'boolean' } } });
try {
  const config = readConfig();
  console.log(
    `${config.name}: ${config.mode}\n${deploymentPlan(config)
      .map((step) => `- ${step}`)
      .join('\n')}`,
  );
  const missing = missingSettings(config);
  if (values['dry-run']) {
    if (missing.length)
      console.log(`\nStill needed:\n${missing.map((item) => `- ${item}`).join('\n')}`);
    console.log(
      config.mode === 'lambda'
        ? '\nRequires AWS CLI credentials and Docker/buildx. GitHub requires an OIDC role. No cloud calls were made.'
        : '\nCreate a Git-linked Render service from render.yaml, configure its runtime secrets, and disable Render auto-deploys. No cloud calls were made.',
    );
  } else {
    if (missing.length) throw new Error(`Missing settings: ${missing.join('; ')}`);
    const sha = releaseCommit();
    if (config.mode === 'lambda') await deployLambda(config, sha);
    else await deployDocker(config, sha);
  }
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
