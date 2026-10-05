# Static frontend + Lambda + Neon

Choose this deployment at setup if you want request-driven compute. Local development
still uses `docker compose up -d db api` and `npm run dev`. The default Docker/Render
mode remains available; switching hosts later does not require different domain code
or a database conversion. PostgreSQL is the only supported database.

This template provisions private S3 storage and CloudFront for the React build, an
API Gateway HTTP API, and Lambda functions sharing one Rust container image. With
optional jobs disabled (the default), there are three functions:

- API: the existing Axum router, with all sessions, authorization, origin checks,
  signed webhooks, and transactions preserved. CloudFront forwards `/api/*`, cookies,
  headers (including Origin and Stripe-Signature), bodies, and query strings without
  caching responses. The SPA fallback applies only to the static behavior.
- Mail: one awaited outbox batch per scheduled invocation. No detached worker that
  might freeze when an HTTP response finishes. Existing claims, retries, and provider
  idempotency keys remain in PostgreSQL.
- Migrations: explicitly invoked using deployment IAM credentials; no public endpoint
  or scheduled event. Uses a direct owner connection. API cold starts do not run DDL.

With `Jobs=sqs` (`npm run setup -- --mode lambda --jobs sqs`), the mail function is
replaced by an SQS-triggered job worker and a recovery dispatcher. The same email
handler runs on Docker and Lambda. See [background jobs](../../docs/background-jobs.md)
for transaction guarantees, retries, setup, and operational limits.

The database, sending domain, Resend account, and Secrets Manager secret are supplied
separately. Nothing is provisioned by cloning or installing this repository.

## Convenience command

`npm run setup` saves your Lambda settings; `npm run deploy` automates image publishing,
stack updates, frontend origin discovery, migration verification, and frontend upload.
See the [deployment command guide](../README.md#lambda-on-aws-with-neon) for the normal
workflow. The steps below remain available for manual deployments and custom domains.

## First deployment

Requires Docker, AWS CLI credentials, and the AWS SAM CLI. Use one AWS region near
your Neon database. The template does not add a VPC or NAT gateway: Lambda connects
to Neon's public TLS endpoint and Resend over HTTPS.

1. Create a Neon project and database. Use its **pooled** connection string for
   `database_url` and the **direct** connection string for `migration_database_url`.
   Require certificate validation: `sslmode=verify-full&sslrootcert=/etc/ssl/certs/ca-certificates.crt` works
   with the bundled libpq/system CA certificates. Do not copy pooler-incompatible
   session settings into the URL. See [Neon connection pooling](https://neon.com/docs/connect/connection-pooling).
   For prototypes both credentials may use the owner; for production, provision a
   restricted runtime role as described in [data patterns](../../docs/data-patterns.md).
2. Create a Secrets Manager JSON secret in the deployment region with keys
   `database_url`, `migration_database_url`, and `resend_key`. Enter the real values
   using a trusted secret-management interface, not a tracked file. Supply its ARN
   to SAM; the deployment identity must be able to resolve the secret (and decrypt
   its KMS key if applicable). CloudFormation resolves these into Lambda environment
   variables; changing a secret alone does not refresh already deployed functions.
3. Create an ECR repository and build/push a single-platform image. Use an immutable
   tag for each revision. Replace the sample account, region, repository, and tag:

   ```sh
   export AWS_REGION=us-east-1
   export APPSHELL_ECR=123456789012.dkr.ecr.us-east-1.amazonaws.com/appshell
   export APPSHELL_TAG=initial
   aws ecr create-repository --repository-name appshell --image-tag-mutability IMMUTABLE
   aws ecr get-login-password | docker login --username AWS --password-stdin "${APPSHELL_ECR%/*}"
   docker buildx build --platform linux/amd64 --provenance=false --push \
     -f Dockerfile.lambda -t "$APPSHELL_ECR:$APPSHELL_TAG" .
   ```

4. Run from the repository root:

   ```sh
   sam validate --lint --template-file deploy/lambda/template.yaml
   sam deploy --guided --template-file deploy/lambda/template.yaml
   ```

   Choose a short stack name such as `appshell-prototype`; provide `ImageUri`,
   `SecretArn`, and your verified `MailFrom`. Accept the IAM creation capability and
   the public API: application authentication is enforced in Rust. Keep `AppUrl`
   at `https://setup.invalid` for this initial provisioning. Do not use the app yet.
   If SAM offers to save configuration, it contains resource IDs/settings only;
   do not put secret values into parameter overrides. New AWS accounts may need a
   concurrency quota increase before reserving the template's function concurrency.

5. Read the stack outputs. Re-run `sam deploy --guided` with the same template and
   set `AppUrl` to the exact `FrontendUrl` output (no trailing slash). This two-step
   setup avoids a CloudFormation dependency cycle between frontend, API, and its
   allowed origin. Keep all other parameter values. Future deployments reuse this
   origin. Custom domains also require an ACM certificate and CloudFront aliases;
   neither is provisioned by the template.
6. Invoke the `MigrationFunction` output synchronously:

   ```sh
   aws lambda invoke --function-name YOUR_MIGRATION_FUNCTION \
     --cli-binary-format raw-in-base64-out --payload '{}' /tmp/appshell-migration.json
   cat /tmp/appshell-migration.json
   ```

   Require no `FunctionError` in the CLI response **and** `{"migrated":true}` in the
   payload before proceeding. A successful CLI exit alone does not prove migration
   success. On updates, migrations must remain compatible with the running version;
   deploy during maintenance if a schema transition needs coordinated rollout.

7. Build and upload the frontend; leave `VITE_API_URL` unset so requests use `/api`.
   Substitute the bucket and distribution outputs:

   ```sh
   npm ci
   npm run build
   aws s3 sync apps/web/dist s3://YOUR_FRONTEND_BUCKET --cache-control 'public,max-age=60'
   aws cloudfront create-invalidation --distribution-id YOUR_DISTRIBUTION_ID --paths '/*'
   ```

   Open `FrontendUrl`. Test signup, verification mail, login/logout, account recovery,
   team permissions, and deep-link refresh. `ApiUrl/ready` checks the database directly.
   Keep previous hashed static assets during rolling updates so open browser tabs
   can finish loading their version. Manage their retention separately.

On subsequent releases, push a new immutable image, update the stack's `ImageUri`,
invoke migrations, then publish the frontend. Keep migration output private. Bootstrap
an installation admin from a trusted machine using the existing local bootstrap
binary and direct database access; there is no public bootstrap Lambda endpoint.

Optional Stripe billing uses the same existing environment variables. Add secret
references for `STRIPE_SECRET_KEY` and `STRIPE_WEBHOOK_SECRET`, plus `STRIPE_PRICE_ID`,
to the API function environment before enabling billing. Register the webhook at
`FrontendUrl/api/webhooks/stripe` and validate with Stripe test mode.

## Cost, latency, and operational limits

Lambda removes the continuously running application server, but does not promise a
zero bill. Account for API Gateway, CloudFront/S3, ECR, Secrets Manager, CloudWatch,
Neon compute/storage, and your mail provider. Check current provider quotas/prices.

The default `MailSchedule=rate(1 minute)` favors usable verification/recovery emails.
**It can keep Neon compute awake even when nobody visits.** Neon normally suspends
idle compute after five minutes; see [scale to zero](https://neon.com/docs/introduction/scale-to-zero).
For low-traffic prototypes, `rate(15 minutes)` creates idle windows but users can wait
up to roughly that interval for mail, with longer delays under backlog or retries.
Opt-in SQS jobs provide immediate publication from API writes plus a recovery poll
every 15 minutes. This reduces idle database polling but does not eliminate it.
Do not simply remove the worker: account emails would remain queued.

With jobs disabled, each invocation claims up to ten legacy messages (or one
remaining shared job after disabling SQS). The worker timeout is 240 seconds, below
the five-minute claim lease; mail provider calls have a 20-second timeout each.
Reserved worker concurrency is one. Failed messages become eligible after five
minutes and stop after ten attempts; monitor exhausted jobs. No failed mail bodies
or action links belong in logs. API reserved concurrency defaults to five and pool
size to two, so budget at least ten API database connections plus workers/migrations,
with headroom for environment replacement and other clients. Use Neon pooling.
Cold starts include TLS/database reconnects and password-hasher initialization.

The frontend bucket is retained on stack deletion. ECR, the secret, and Neon are
external and also need deliberate cleanup. Review CloudWatch retention and alerts
before sustained use. Local validation and CI checks do not deploy. The explicitly configured release command
and optional GitHub Deploy job do provision/update AWS resources; they require your
credentials and the saved deployment settings.

## DynamoDB alternative

New installations can choose `npm run setup -- --mode lambda --storage dynamodb`.
This provisions DynamoDB and uses SES, with SQS jobs enabled by default. It does
not need Neon or a Resend secret. The same SAM template supports both backends;
the image must be built for the matching storage feature. See the
[DynamoDB guide](../../docs/dynamodb.md) for local development, LocalStack SAM E2E,
admin bootstrap, persistence guarantees, costs, and deployment prerequisites.
The PostgreSQL instructions above remain applicable to the default backend.

References: [AWS Rust runtime](https://docs.aws.amazon.com/lambda/latest/dg/lambda-rust.html),
[Lambda lifecycle and freezing](https://docs.aws.amazon.com/lambda/latest/dg/lambda-runtime-environment.html),
[SAM scheduled invocations](https://docs.aws.amazon.com/serverless-application-model/latest/developerguide/sam-property-function-schedule.html).

## Native ARM builds and cloud smoke tests

Lambda setup accepts `--architecture arm64` (default: `x86_64`). The release tool
builds the matching Docker platform and sets SAM's `Architecture` parameter; image
tags distinguish ARM from AMD64. On Apple Silicon this avoids QEMU compilation.
For manual deployment, pair `--platform linux/arm64` with `Architecture=arm64`.
CI builds both Lambda storage variants on native AMD64 and ARM runners.

`AWS_PROFILE=personal npm run test:aws -- --architecture arm64` creates a temporary
DynamoDB SAM stack, tests the deployed CloudFront app and Streams/SQS pipeline,
and removes its retained test resources. It incurs normal AWS charges and skips
SES delivery. See the [AWS smoke-test guide](../../docs/dynamodb.md#opt-in-aws-smoke-test)
for requirements, coverage, and cleanup. Normal development uses DynamoDB Local
and Mailpit without an AWS account or LocalStack token.
