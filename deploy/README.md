# Set up and deploy a new app

Create your app repository from this starter, install dependencies, and run:

```sh
npm ci
npm run setup
```

Choose `docker` (Render/PostgreSQL) or `lambda` (AWS with PostgreSQL or DynamoDB), an app name, and whether GitHub
should deploy automatically after checks pass. Setup saves **`appshell.deploy.json`**:
commit this non-secret file with your app. Resource IDs can be added later by running
setup again. No cloud requests are made during setup. Product branding still lives
in `apps/web/src/config/brand.ts`; this command does not rename your packages/product.

```sh
npm run deploy:plan  # Show saved settings, missing prerequisites, and release steps
npm run deploy      # Actually deploy the current committed revision
```

`deploy:plan` makes no cloud calls and requires no credentials. `deploy` requires a
clean working tree, including a committed configuration. Run `npm run verify` before
a local release; GitHub releases run the required checks automatically. Local deploys
are not serialized with GitHub: use one release channel at a time.

For PostgreSQL storage, both hosting modes use the same local setup:

```sh
docker compose up -d db api
npm run dev
```

For a new AWS-only installation, choose `--mode lambda --storage dynamodb`.
[DynamoDB setup](../docs/dynamodb.md) includes local Docker and optional LocalStack
SAM testing, SES prerequisites, and operational differences. Existing configurations
default to PostgreSQL. Storage changes require a separate data migration.

## Docker on Render

Save the choice before or after creating your hosting account:

```sh
npm run setup -- --mode docker --name my-app
```

Connect your new Git repository to a Render Blueprint using `render.yaml`. Set its
service name and production runtime values: `APP_URL`, `DATABASE_URL`, `MAIL_FROM`,
and `RESEND_API_KEY`. The blueprint supplies `APP_ENV=production` and `MAIL_MODE=resend`.
Postgres may be Neon or another provider. The container includes React, the Rust API,
and mail delivery; migrations run on startup.

The blueprint disables commit-triggered Render auto-deploys. When using GitHub to
control releases, also set the Blueprint's **Auto Sync** setting to **No**, so edits
to `render.yaml` cannot independently trigger a deployment before checks finish.
Creating the initial Blueprint itself deploys the service; later releases use the
commands/workflow below. See [Render Blueprint behavior](https://render.com/docs/infrastructure-as-code).

Copy the service ID from Render, then update your saved settings:

```sh
npm run setup -- --mode docker --service-id srv-YOUR_SERVICE_ID
```

Provide `RENDER_API_KEY` through your shell's environment or credential manager
and run `npm run deploy`. The command requests a build of the **current Git SHA**
from Render and waits for it to become live. Push that commit to Render's linked
repository first. A failed build or deployment exits unsuccessfully; runtime secrets
stay in Render. No local Docker image upload is necessary for this Git-linked path.
The manual `Dockerfile` deployment remains available for other container hosts.

For GitHub releases, add `RENDER_API_KEY` as a secret in the repository or its
`production` environment. No Render token is stored in `appshell.deploy.json`.

For optional Postgres jobs, create the Blueprint using **`deploy/render.jobs.yaml`**
as its path. It adds a separately billed background worker and configures the API
and worker to share Postgres. Fill the same database, origin, sender, and mail
credentials on both services. Save both service IDs:

```sh
npm run setup -- --mode docker --jobs postgres \
  --service-id srv-YOUR_WEB_ID --worker-service-id srv-YOUR_WORKER_ID
```

`npm run deploy` updates the worker first, waits for it to become live, then updates
the API to the same commit. Setup only saves settings: it does not create Render
services or change their runtime environment. To upgrade an existing Blueprint,
review and apply the optional blueprint's service/environment changes in Render
once, and retain the Auto Sync/auto-deploy settings described above. See
[background job operation and local development](../docs/background-jobs.md).

## Lambda on AWS with Neon

First create a Neon database and verified Resend sender. In the deployment AWS
region, create a Secrets Manager JSON secret containing `database_url` (pooled Neon
URL), `migration_database_url` (direct migration/owner URL), and `resend_key`.
Use the TLS settings and database roles described in the [Lambda guide](lambda/README.md).
Choose an app name unique within your AWS account/region; it names the stack and ECR
repository. The convenience command supports the template's generated CloudFront
hostname in commercial AWS regions. Custom-domain deployments use the manual guide.

```sh
npm run setup -- --mode lambda --name my-app --region us-east-1 \
  --secret-arn arn:aws:secretsmanager:us-east-1:123456789012:secret:my-app-AbCdEf \
  --mail-from 'My App <hello@example.com>'
npm run deploy:plan
```

Commit your configuration and changes. With Docker/buildx running and AWS CLI v2
credentials configured, run `npm run deploy`. SAM CLI is not required for this path;
the command submits the existing SAM template through CloudFormation. It:

1. Confirms your AWS account matches the secret ARN and builds the React frontend.
2. Creates the ECR repository if missing and publishes a Linux amd64 Lambda image
   tagged with the Git SHA. Re-runs reuse an image already published for that SHA.
3. Creates/updates the stack, discovers the frontend URL, and sets its allowed origin
   automatically. The manual two-deployment bootstrap is handled for you.
4. Invokes migrations synchronously and verifies both the Lambda result metadata and
   `migrated: true`. Migration failures stop frontend publication.
5. Uploads frontend assets, publishes `index.html` last, and waits for CloudFront cache
   invalidation. Older hashed assets remain available to open browser tabs.

The same command handles subsequent releases. Backend updates happen before the
explicit migration invocation, so schema changes must remain backward compatible
with the running version. This is not an atomic release or an automatic rollback:
if a later step fails, fix the cause and re-run the command. Secrets Manager value
changes alone do not refresh Lambda environment values; follow the manual guide's
secret-rotation note and force a function configuration update when rotating secrets.

Mail polling defaults to one minute, which can keep Neon awake. Choose a different
tradeoff with `--mail-schedule 'rate(15 minutes)'`; email delivery can then take that
long or longer under backlog. See the [cost and latency notes](lambda/README.md#cost-latency-and-operational-limits).

To enable the shared email handler through SQS, add `--jobs sqs` to setup. Deployment
then provisions the SQS queue, dead-letter queue, worker Lambda, and recovery
dispatcher instead of the legacy scheduled mail Lambda. The API attempts immediate
publication after committing a write; recovery polls Postgres every 15 minutes.
`--mail-schedule` applies only when jobs are disabled. The deployment IAM role also
needs SQS provisioning permissions when this option is enabled.

For GitHub releases, configure an AWS OIDC provider and deployment role using
[GitHub's AWS OIDC guide](https://docs.github.com/en/actions/how-tos/secure-your-work/security-harden-deployments/oidc-in-aws).
The role's trust policy must match your repository and the workflow's **production**
environment, with `aud=sts.amazonaws.com` and
`sub=repo:YOUR_OWNER/YOUR_REPOSITORY:environment:production`. Limit that GitHub
Environment to your default branch. Grant the deployment role the permissions needed
for this stack: CloudFormation, its IAM roles/pass-role, Lambda, API Gateway,
EventBridge, S3, CloudFront, ECR, SQS when jobs are enabled, and DynamoDB when selected.
PostgreSQL deployments also need resolution/decryption of the configured secret.
Scope those permissions to your app; runtime Lambda roles remain separate.

Save the role ARN:

```sh
npm run setup -- --mode lambda --role-arn arn:aws:iam::123456789012:role/my-app-deploy
```

GitHub obtains temporary AWS credentials via OIDC; do not add long-lived AWS keys as
repository secrets. Local deployment continues to use your local AWS CLI identity.
The PostgreSQL database/secret (when selected), verified sending identity, and deployment
identity are deliberate one-time prerequisites; setup does not create accounts, credentials, or IAM trust relationships.

## GitHub release behavior

The **Checks** workflow runs formatting, types/lints, architecture, contracts,
unit/integration/browser/mail/Storybook tests, all container builds, and infrastructure
validation. Only then may it call the reusable **Deploy** workflow. Pull requests and
non-default branches never deploy. A stale default-branch revision is skipped before
release configuration. Running releases are not canceled by newer pushes.

Automatic deployment is opt-in:

```sh
npm run setup -- --mode docker --auto-deploy
# Or --mode lambda; existing settings are preserved for the same mode.
```

Commit and push the configuration. Subsequent default-branch pushes deploy after
checks pass. To turn this off, use `--no-auto-deploy`. With automatic deployment off,
open **Actions → Checks → Run workflow**, select your default branch, and check
**deploy**. This runs the checks before releasing the saved mode. Both paths use the
`production` GitHub Environment, including any protection rules you configure there.

There is no configuration file committed in the starter itself, so a fresh clone
never deploys just because it is pushed. Changing the saved mode does not delete or
migrate your existing hosting resources. Retire the old hosting separately after
verifying the new deployment. Hosting changes retain the selected storage backend.
Switching storage is a separate data migration, not a hosting change.

Lambda configurations also accept `--architecture arm64` for native Apple Silicon
builds; `x86_64` remains the default. Docker and SAM use the same architecture.
See [DynamoDB development and AWS smoke tests](../docs/dynamodb.md) for the
account-free local workflow and the opt-in temporary AWS validation command.
