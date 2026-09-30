# Optional background jobs

Email is the first shared job handler. Enable jobs once during app setup; keep the
same Rust services, database, and email templates in either deployment mode.
The default is `disabled`: email keeps its existing transactional outbox worker,
and no SQS resources or additional Docker worker are required.

| Setting    | Runtime                                  | Delivery                                            |
| ---------- | ---------------------------------------- | --------------------------------------------------- |
| `disabled` | Combined Docker or scheduled mail Lambda | Existing mail outbox                                |
| `postgres` | Docker API plus separate Rust worker     | Postgres claims with `SKIP LOCKED`                  |
| `sqs`      | API Lambda plus SQS job Lambda           | SQS notifications referencing durable Postgres jobs |

## Setup

For Docker/Render, use `deploy/render.jobs.yaml` instead of the default blueprint
and configure both services with the same Postgres database and email settings.
The API uses `RUN_MODE=api`, the worker `RUN_MODE=worker`; both set
`JOB_BACKEND=postgres`. The worker is a separate process of the same Rust binary
and image. Render workers require a paid instance. Save the service IDs:

```sh
npm run setup -- --mode docker --jobs postgres \
  --service-id srv-YOUR_WEB_ID --worker-service-id srv-YOUR_WORKER_ID
npm run deploy:plan
```

After committing and pushing your revision, `npm run deploy` updates the worker
before the API. It does not provision the Render services or edit their environment.

For Lambda/Neon, add the option to your existing configuration:

```sh
npm run setup -- --mode lambda --jobs sqs
npm run deploy:plan
```

After committing, `npm run deploy` applies the SAM `Jobs=sqs` parameter. This creates
the queue, dead-letter queue (DLQ), consumer Lambda, and recovery Lambda. IAM send
permissions apply to the API and dispatcher; the consumer gets queue read permissions.
The queue URL is injected automatically. Manual SAM deployments use the same parameter.

Local development for either target uses Postgres and Mailpit; no AWS account or
SQS emulator is needed:

```sh
docker compose -f compose.yaml -f compose.jobs.yaml up -d db api worker
npm run dev
```

Restart both `api` and `worker` with the same Compose files after Rust edits. The
API does not deliver mail itself in this configuration. `npm run test:mail` tests
all email types through the separate worker and Mailpit. To return to the default
local setup, stop the optional worker with these files, then run
`docker compose up -d api` to restore the combined API configuration.

ECS/Fargate can run this same Docker image as two services with those `RUN_MODE`
values and one Postgres database. An ECS provisioning/deployment target is not
included in the convenience command.

## Commit, publish, execute

Account changes, the email outbox record, and `background_jobs` commit in one
Postgres transaction. A rollback leaves no mail or job behind. The job payload is
versioned (`mail.v1`) and contains only the mail record ID. Mail bodies and action
links remain in the protected mail outbox, never in queue notifications. Job payloads
are redacted in the companion history table and scrubbed on success.

The Docker worker claims runnable jobs, calls the shared handler, and records the
outcome. Each claim has a two-minute lease and an incrementing version: an old
worker cannot complete a newer worker's claim. Handlers have a 60-second timeout.
No database transaction stays open while contacting an external provider. SIGTERM
stops new claims and waits for the active operation; forced termination recovers
through lease expiry.

On Lambda, the API awaits an attempt to publish committed job IDs to SQS after a
write, with a three-second budget. A publication error never reverses a committed
API response. The durable outbox recovers failed or interrupted publication after
a 15-minute publication lease. A scheduled dispatcher checks every 15 minutes;
missed publications can therefore wait roughly 15–30 minutes before recovery,
and longer under backlog. It also drains legacy mail created before jobs were enabled. It publishes up to
ten IDs per invocation, as does the API fast path. This is intentionally a small
prototype queue: a large backlog needs a larger bounded dispatcher or shorter
schedule. Delayed jobs depend on these checks, so their execution time is a lower
bound, not an exact appointment. Delays are limited to one week.

SQS notifications contain only `{"version":1,"job_id":"..."}`. The consumer uses
the same Postgres execution lease and handler as Docker. Completed or soft-deleted
jobs are acknowledged without repeating delivery. Invalid, busy, not-yet-due, and
failed jobs return a partial batch failure. The template uses one record per
invocation, a 90-second Lambda timeout, 540-second queue visibility, and maximum
concurrency two. See AWS's [SQS configuration](https://docs.aws.amazon.com/lambda/latest/dg/services-sqs-configure.html)
and [partial failure behavior](https://docs.aws.amazon.com/lambda/latest/dg/services-sqs-errorhandling.html).

Both modes provide **at-least-once execution**, not exactly-once external effects.
Handlers must be idempotent. Email skips already delivered records and sends Resend
the mail UUID as its idempotency key. A provider-accepted send followed by a crash
can still duplicate mail outside the provider's deduplication window; SMTP has no
equivalent deduplication guarantee.

## Failures and operations

Postgres job attempts back off from five seconds to five minutes and stop after
ten claims, including crashed attempts. SQS redelivery also depends on its longer
visibility timeout. `failed_at` and a static `error_code` mark terminal failure;
no provider response or payload is copied into the error field. Exhausted jobs
are not automatically resurrected. SQS additionally redrives repeatedly failed
notifications to the DLQ after ten receives. Both queues retain messages for 14 days.

Monitor runnable job age, `failed_at`, worker errors, and SQS DLQ depth. Inspect
`background_jobs` for shared job attempts, and `mail_outbox` for mail delivery;
`mail_outbox.attempts` applies only to legacy jobs. Recovery is deliberately manual:
fix the cause, confirm the operation remains valid, then in a trusted database
maintenance transaction reset the selected failed job's `attempts` to zero,
`failed_at`, `locked_until`, and `dispatched_until` to NULL, and `available_at` to
now. Preserve its ID and lease version. Attribute the maintenance transaction as
documented in [data patterns](data-patterns.md). DLQ redrive alone cannot reset a
terminal Postgres job. For expired verification/reset links, request a fresh email
through the application instead of replaying an obsolete action.

Turning jobs off stops creating new shared jobs. Workers still drain committed
jobs and legacy mail. On Render, restore `RUN_MODE=combined` and
`JOB_BACKEND=disabled` on the web service before retiring the worker; saving
`--jobs disabled` alone does not change Render runtime settings. On Lambda, deploying
`Jobs=disabled` restores the legacy scheduled worker, which also drains remaining
shared jobs one per invocation. CloudFormation **retains** SQS queues when disabling
jobs or deleting the stack: record their URLs and remove them deliberately after
draining/auditing. Re-enabling can create new queues; the Postgres outbox remains
the source of pending work.

SQS removes the continuous application worker, but recovery still wakes Neon
periodically. Queue polling, Lambda, logs, and database usage can incur charges.
This is not a workflow engine: no cron registry, dependencies, priorities, UI,
heartbeats, or long-running tasks are included. Keep handlers below 60 seconds;
use a separate orchestration/task service for longer work.

## Add a handler

1. Add a versioned, typed variant in `infrastructure/job_queue.rs` containing the
   minimum identifiers required. Do not serialize credentials or request bodies.
2. Enqueue through `job_queue::enqueue` inside the authorized service's existing
   `UnitOfWork::transaction`, with any related writes on that same connection.
   Optional business features must handle `JobBackend::Disabled` explicitly.
3. Dispatch the variant in `jobs.rs` to the owning context's service. The service
   must recheck current tenant scope, authorization, and invariants. An old queued
   request must not bypass revoked permissions. Do not call another context's
   repository from the handler or add transport-specific business logic.
4. Make external effects idempotent and test rollback, duplicate execution, retries,
   and tenant isolation. Keep old payload versions executable through rolling
   releases and until pending work is drained.

`npm run test:rust` tests real Postgres rollback, concurrent claims, lease expiry,
retry exhaustion, redacted history, mail cleanup, SQS partial failures, and SDK
publication/recovery against a local HTTP stub. Live AWS IAM/event-source behavior
and real Resend delivery still require a deployment smoke test.
