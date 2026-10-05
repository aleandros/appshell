# DynamoDB storage and AWS SAM

New applications can choose DynamoDB at setup. All application persistence uses
that backend: users, administrators, sessions, action tokens, rate limits,
organizations, memberships, invitations, billing state, mail, jobs, and history.
PostgreSQL remains the default. The React app, API contract, domain policies, and
context services are shared; the backend is selected when building the Rust binary.

## Start a new app

```sh
npm ci
npm run setup -- --mode lambda --storage dynamodb --name my-app --region us-east-1
# Local development needs Docker, not an AWS account or LocalStack subscription:
docker compose -f compose.dynamodb.yaml up -d
npm run dev
```

Open http://localhost:5173. The API uses port 8080, DynamoDB Local uses 8000,
and Mailpit uses http://localhost:8025. Stop the PostgreSQL API/Mailpit containers
first if they already occupy those ports (`docker compose stop api mailpit`).
Restart the DynamoDB API after Rust changes. Its first build can take several minutes.
Local data lives in a named Docker volume and survives container restarts.

With native Rust, start only `dynamodb` and `mailpit`, then run:

```sh
export AWS_ACCESS_KEY_ID=local AWS_SECRET_ACCESS_KEY=local AWS_REGION=us-east-1
export AWS_EC2_METADATA_DISABLED=true
export DYNAMODB_TABLE=appshell-local DYNAMODB_ENDPOINT_URL=http://localhost:8000
export DYNAMODB_CREATE_TABLE=true
export MAIL_FROM='AppShell <hello@appshell.test>'
cargo run --locked --no-default-features --features dynamodb --bin appshell-api
```

These are dummy local credentials. Production uses Lambda execution roles.
`DYNAMODB_CREATE_TABLE=true` only creates a missing table when a development
endpoint is configured and `APP_ENV` is not production. SAM provisions cloud tables;
production startup never creates them. Readiness checks access to the selected table.
The private migration Lambda validates its key/index schema. Future data-format
changes need explicit versioned migration code; changing a SAM table definition
alone does not migrate existing records.

Storage is an installation choice, not a connection-string switch. Setup and the
release tool reject an accidental change of backend for an existing app/stack.
Moving existing data requires an explicit export, transformation, validation, and
cutover procedure; that migration is not included. Docker/Render setup currently
supports PostgreSQL; DynamoDB's production deployment target is Lambda/SAM.

## Persistence guarantees and access patterns

`infrastructure/repositories/dynamodb` implements the same context-owned repository
operations as PostgreSQL. It runs on blocking workers and uses the async AWS SDK
through the Tokio runtime handle, so existing synchronous transaction closures and
Argon2 work do not block async workers. No provider calls happen inside a database
transaction. Build DynamoDB with `--no-default-features --features dynamodb`; add
`lambda,ses` for the AWS deployment. That build does not depend on Diesel or libpq.

The table has `pk`/`sk` string keys and a `listing` index on `gpk`/`gsk`.
Records contain a JSON document plus an optimistic version. UUIDs originate only
in the persistence adapter, outside the pure domain. The main access patterns are:

| Operation                                   | Access                                                                    |
| ------------------------------------------- | ------------------------------------------------------------------------- |
| Account, session, action, subscription, job | Consistent primary-key read                                               |
| Email uniqueness                            | Transactionally reserved email key                                        |
| Team and pending invitations                | Consistent organization partition query                                   |
| Organizations for a user                    | Consistent user partition query                                           |
| Change history                              | Consistent record-history partition query                                 |
| Pending mail/jobs                           | Sparse listing index, followed by consistent reads and conditional claims |
| Admin substring search                      | Account listing index traversal and application filtering                 |

Admin search preserves existing substring/offset behavior, but reads the account
index and has linear cost. It is eventually consistent. It is intended for small
installations; add a purpose-built search index and cursor API before using it for
large directories. It is never used for authorization or uniqueness. Team queries
and seat decisions use primary data. Do not substitute an eventually consistent
index for a security or entitlement check.

A unit of work buffers writes and records versions of consistent reads. Commit
uses one `TransactWriteItems` request with conditional writes/read checks and all
history/outbox entries. Organization mutations update an aggregate revision, which
protects partition-query decisions from concurrent membership/invitation changes.
User records are also checked when determining occupied seats. Conflicts fail with
HTTP 409 `write_conflict`; callers can retry the operation. The adapter does not
replay closures containing random tokens or password hashing automatically.

DynamoDB transactions are limited to 100 items and 4 MB; each history entry also
uses an item. Oversized operations fail atomically with `transaction_limit` when
the item limit is exceeded. Keep new use cases bounded and test their largest
supported aggregate. Large administrative restorations can leave organizations
over their seat limit, as in PostgreSQL; further joins still check entitlements.

Session and action revocation uses versions on the account, so revoking many
credentials is constant-size. Per-purpose action generations make reissued links
invalidate their predecessors. Expiration is checked by the application on every
use. TTL is deliberately disabled: asynchronous physical deletion must not replace
expiry checks or violate retained history/soft-deletion rules.

Every adapter write includes an append-only, redacted history entry with its actor.
Passwords, action payloads, mail bodies, and job/checkout payloads are redacted.
Soft deletion preserves records; memberships use stable slots whose removal and
restoration are audited. Email reservations remain held while an account, including
a deleted account, uses the address. History is committed with the business change.
DynamoDB has no PostgreSQL-style triggers: these guarantees are enforced by the
adapter, conditional expressions, tests, and restricted IAM. A privileged operator
can bypass application conventions. Runtime roles have no DeleteItem/Scan permission;
do not grant direct application writers outside this adapter.

## AWS deployment and email

```sh
npm run setup -- --mode lambda --storage dynamodb --name my-app \
  --region us-east-1 --mail-from 'My App <hello@example.com>'
npm run deploy:plan
# From a clean committed checkout, with deployment AWS credentials:
npm run deploy
```

DynamoDB setup defaults to SQS jobs. The existing release command builds the
frontend, publishes a backend-specific immutable image, applies the SAM template,
validates storage through the private migration function, and publishes assets.
It creates ECR as needed. SAM provisions the on-demand encrypted table with point-in-time
recovery, Lambda/API Gateway, SQS/DLQ, stream dispatch, and S3/CloudFront frontend.
No Neon database or Resend secret is needed for DynamoDB mode. The deployment
role needs DynamoDB and SQS provisioning permissions in addition to the existing
stack permissions.

Lambda supports both `x86_64` (the default) and `arm64`. On Apple Silicon, select
`--architecture arm64` during setup to build natively without QEMU. Setup persists
this choice; deploy uses matching Docker platforms, architecture-specific image
tags, and SAM's `Architecture` parameter. An ARM image must use `Architecture=arm64`.
For AMD64 releases from an ARM machine, use a native AMD64 Docker builder/CI runner
if QEMU crashes. This avoids emulation; it does not repair QEMU itself.

```sh
npm run setup -- --mode lambda --storage dynamodb --name my-app --architecture arm64
```

Email uses SES over HTTPS with the existing escaped HTML/plain-text templates.
Verify the sender in the deployment region and obtain SES production access before
sending to arbitrary recipients. DNS verification and AWS sandbox approval cannot
be completed merely by deploying SAM. SES does not provide Resend's idempotency-key
behavior: an interruption after SES accepts mail but before recording completion
can cause a duplicate. Delivery remains at least once.

New job records and mail commit together. Stream INSERT events publish committed
job IDs to SQS. Workers use conditional execution leases and completion records.
The scheduled recovery dispatcher republishes missed or delayed work every 15
minutes, including events lost beyond stream retention. Failed jobs retain their
error code; monitor them and the SQS DLQ. No email body or action link goes into SQS.
Scheduled recovery remains necessary. With jobs disabled, scheduled mail polling
is used instead. DynamoDB storage, reads/writes, history, indexes, backups, and the
other AWS services are billed; this mode does not promise a zero bill.

Tables, queues, and the frontend bucket are retained on stack deletion. Review
retention, recovery, and deliberate teardown with a trusted operator.

To bootstrap administration, use the existing private password prompt. The script
reads the saved setup storage choice, or accepts an explicit override:

```sh
python3 scripts/bootstrap-admin.py --storage dynamodb
# Native Rust instead of Compose; set table/region/credentials first:
python3 scripts/bootstrap-admin.py --local --storage dynamodb
```

Credentials travel over stdin, never command arguments or environment variables.
Use a trusted environment with appropriate table IAM permissions for production.
There is no public bootstrap endpoint, and bootstrap is permanently disabled once
the first administrator exists. See [data patterns](data-patterns.md).

## Validation

```sh
# Docker required; uses native Rust when available, otherwise the Compose runner.
# Creates a uniquely named test table, removed after success:
npm run test:dynamodb
# Running DynamoDB API + Mailpit:
npm run test:e2e
npm run test:mail
# Compile/lint both storage implementations; PostgreSQL regression tests separately:
npm run check
npm run test:rust
```

The DynamoDB integration suite exercises rollback, uniqueness, competing token
consumers, session/action revocation, tenant isolation, history redaction/actors,
soft deletion/restoration, job leases, and one-time admin bootstrap. CI also runs
the existing browser and real Mailpit delivery tests against DynamoDB and builds
both Lambda images. PostgreSQL tests retain their separate disposable schemas.

For broader local SAM integration, use a LocalStack plan with ECR and API Gateway
HTTP API support (currently Base or higher, or an eligible Student plan). Hobby
does not include those services. Install Docker, AWS CLI, SAM CLI, and LocalStack's
`lstk` CLI, then set `LOCALSTACK_AUTH_TOKEN` privately and run:

```sh
npm run dev:sam
# Or deploy and run browser + captured-mail tests automatically:
npm run test:sam
```

The script deploys the production template's backend into LocalStack using dummy AWS
credentials and an explicit local endpoint. It prints `API_PROXY_TARGET` and
`API_TEST_URL` commands for browser and Mailpit tests. Vite supplies the frontend;
`LocalDevelopment=true` omits CloudFront/S3 frontend resources and enables local
SMTP. This tests Lambda, DynamoDB, Streams, SQS, and mail integration, not CloudFront.
Ports 4566, 8025, and 1025 must be free. Stop any existing Vite server before
`test:sam`; it starts its own server so tests cannot silently hit another backend. Set `LOCALSTACK_IMAGE` to a tested image tag
for reproducible team/CI use. Current LocalStack images require authentication and
service availability depends on the selected plan/version. As of March 23, 2026,
LocalStack requires an account token even on its free Hobby plan. Hobby is for
non-commercial use; commercial development needs an appropriate plan. See
[LocalStack pricing](https://www.localstack.cloud/pricing) and
[service coverage](https://docs.localstack.cloud/aws/licensing/). Ordinary app development,
browser tests, and Mailpit mail tests use DynamoDB Local and need no LocalStack
account, subscription, token, or real AWS credentials.

The SAM runner defaults to the Docker engine's native architecture. Override with
`npm run test:sam -- --architecture x86_64` or `arm64` when testing another builder.

## Opt-in AWS smoke test

```sh
AWS_PROFILE=personal npm run test:aws -- --region us-east-1 --architecture arm64
```

This command **creates billable AWS resources** in a new, uniquely named
`appshell-smoke-*` stack. It uses the current working tree, independent of the saved
app configuration; normal releases still require a clean commit. It deploys the
same SAM template with production settings, validates the schema, and runs desktop
and mobile browser tests against the actual CloudFront URL. It also inserts an
isolated replay fixture outside the pending-jobs index, proving DynamoDB Streams →
dispatcher → SQS → worker → atomic job completion without the API publication fast
path or scheduled recovery. The fixture references already-delivered mail, so SES
is not called for that check. Browser signup uses reserved `.invalid` recipients;
unverified SES delivery may fail and remain queued, which is not a delivery test.
The admin browser tests use fixtures; backend admin behavior has separate
DynamoDB integration coverage.

The runner needs Docker, AWS CLI, installed project dependencies and Playwright
Chromium, plus deployment and cleanup permissions. It defaults to the Docker
engine's architecture. SAM CLI is not required: AWS CloudFormation applies the
SAM transform. `AWS_PROFILE` selects credentials without copying secrets into app
configuration. It never verifies senders or requests SES production access.

On success or test failure it deletes its own stack, retained DynamoDB table,
SQS queues, frontend bucket/assets, function logs, and ECR repository. It writes a
report with exact resource IDs and cleanup status under the printed temporary
directory. If interrupted or cleanup fails, use that report and the exact smoke
stack name to finish cleanup; do not delete resources belonging to your app.
LocalStack success does not prove AWS parity, and this smoke test intentionally
leaves SES permissions/deliverability unverified. Run Mailpit tests separately.

References: [DynamoDB Local](https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/DynamoDBLocal.html),
[transaction limits](https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/transaction-apis.html),
[LocalStack SAM](https://docs.localstack.cloud/aws/connecting/infrastructure-as-code/aws-sam/),
[SES verification](https://docs.aws.amazon.com/ses/latest/dg/verify-addresses-and-domains.html).
