# Administration and persisted data

Every application table uses a PostgreSQL-generated UUID primary key, soft deletion,
and an append-only companion history table. Migration `00000000000004` upgrades
existing installations without dropping their data. Existing UUIDs remain stable;
previous natural/composite keys become business constraints. Stripe event IDs are
stored in `billing_events.provider_event_id`, separately from the database UUID.

## Administrator realm

Open `/admin/login` to enter the administration console. Administrator accounts
live in `admin_accounts`, independently of application `users` and organization
roles. Their hashed opaque sessions live in `admin_sessions`, expire after eight
hours, and use a separate HttpOnly, SameSite=Strict cookie scoped to `/api/admin`.
Production adds Secure. Origin checks and persistent login rate limits apply to
this realm too. An organization admin has **no** installation administration rights.

Provision the first account from the repository root:

```sh
python3 scripts/bootstrap-admin.py
```

The script prompts for email, name, and a confirmed password without echoing it.
It sends credentials to the Rust binary over stdin, never command arguments,
logs, or environment variables. Python 3 and the running Compose database are
required. With native Rust, export `DATABASE_URL` and pass `--local`. For another
deployment, run the `bootstrap-admin` binary from a trusted management environment
with database connectivity and the same JSON stdin contract (`email`, `name`,
`password`). Build it with `cargo build --locked --bin bootstrap-admin`; the
production web image does not bundle management binaries.

Bootstrap takes a database transaction lock and succeeds only if **no administrator
has ever been created**, including deleted accounts. It cannot overwrite credentials
or create a second administrator. Use the console to create and manage subsequent
administrators. There is no public registration or user-to-admin promotion endpoint.
Administrators currently have equal installation-wide authority. Keep future
permission decisions in `apps/domain/src/admin.rs`; add use cases behind the admin
context, HTTP adapter, and frontend feature facade.

The console searches and paginates users and administrators, including deleted
accounts. It supports name/email corrections, active/suspended status, soft deletion,
restoration, and user password-reset emails. It also lets administrators set another
administrator's password. Changes revoke the affected account's sessions; user
changes revoke outstanding action tokens as well. User email corrections clear
verification, notify the old address, and queue verification to the new address when
active. Password-reset emails use the transactional outbox and existing single-use
flow; no plaintext password or reset token is returned to the administrator.

An administrator cannot suspend or delete their own account. Administrator-management
writes serialize and recheck the acting account under lock, preventing two admins
from concurrently disabling each other and leaving no active account. Editing your
own admin profile/password signs you out. There is no public admin password recovery;
another active administrator can reset it. If all administrators lose their credentials,
recovery requires a trusted database operator. MFA and fine-grained admin roles remain
extension points.

Suspended/deleted users cannot log in, use old sessions, or consume action tokens.
Soft deletion preserves related data and memberships; restoration makes retained
memberships usable again. Deleted users are hidden from team listings and excluded
from seat counts; suspended users retain their reserved seats. Administrative
restoration can therefore put a workspace over its seat limit, just as a subscription
downgrade can; subsequent invitations/joins still enforce the limit. Organization
ownership is retained. An administrator should resolve ownership operationally before
disabling a workspace's only owner.

## Adding a model

Create a new migration. Use an `id uuid PRIMARY KEY DEFAULT gen_random_uuid()` and
call `protect_model` after creating the table:

```sql
CREATE TABLE projects (
 id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
 organization_id uuid NOT NULL REFERENCES organizations,
 name text NOT NULL,
 created_at timestamptz NOT NULL DEFAULT now()
);
SELECT protect_model('projects');
-- If a model contains secrets, list every secret column explicitly:
-- SELECT protect_model('provider_credentials', ARRAY['access_token','refresh_token']);
```

`protect_model` installs:

- Nullable `deleted_at`, `deleted_by` (UUID), and `deleted_by_kind` columns.
- A write trigger that generates INSERT IDs in Postgres, even if a caller supplied
  one, rejects ID changes, and sets deletion attribution automatically.
- A statement trigger that rejects DELETE and TRUNCATE, even through direct SQL.
- A `<model>_history` table and an INSERT/UPDATE history trigger.
- A history trigger rejecting UPDATE, DELETE, and TRUNCATE.

Use `INSERT ... RETURNING id` to obtain a new ID before inserting related rows.
Never manufacture model IDs in Rust or JavaScript. Tokens are a separate concern:
cryptographically random bearer tokens still originate in the crypto adapter. Provider
identifiers and composite business keys belong in separate uniquely constrained columns.

After embedded migrations, Docker startup (or the explicit Lambda migration invocation)
runs `validate_model_conventions()`. It
checks every table in the application's current schema for its generated UUID primary
key, deletion columns, required enabled triggers, and companion history table. A new
model that omits `protect_model` prevents startup and fails the migration integration
test. Keep unrelated extension/tool tables in a different schema. Only Diesel's own
migration bookkeeping and recognized companion history tables are exempt from model
soft-deletion/history recursion; history itself is append-only with generated UUIDs.
The architecture test additionally rejects hard-deletion SQL in repositories and
application UUID generation outside token generation.

## Soft deletion and uniqueness

Repositories explicitly filter `deleted_at IS NULL` on ordinary reads, joins,
counts, locks, and writes. SQL does not automatically hide deleted rows: administrative
search/history and restoration intentionally need them. Review new queries and add
lifecycle tests for this rule. In particular, check joined parents, authorization,
seat counts, and provider/job claim queries; a filter on just the leaf table is not
always sufficient. Soft deletion does not cascade automatically. Each use case must
choose whether related records remain available, become inaccessible through the
parent, or are explicitly soft-deleted in the same transaction.

Revoke a record with `UPDATE ... SET deleted_at=now() WHERE ... AND deleted_at IS NULL`.
Never issue DELETE. The trigger fills `deleted_by` from the transaction actor and
preserves the original deletion time/actor on subsequent writes. Explicit restoration
sets `deleted_at=NULL`; the trigger clears deletion attribution and history records
the restoration. Reactivation never restores revoked sessions or action tokens.

User and administrator emails remain globally reserved, even when deleted; restoring
an account is preferable to silently creating a different identity at the same email.
Memberships and invitations use partial uniqueness on active rows so removal and
rejoining/reinvitation create distinct records with distinct histories. Action-token
reissuance soft-deletes the preceding token and inserts a new record. Operational
singletons (rate-limit windows, billing leases, checkout attempts) may explicitly
reactivate/update their existing row, with history, while immutable provider event
keys stay reserved for replay protection.

## Actors, history, and transactions

Use the existing `UnitOfWork::transaction` and call `c.actor("user", id)` or
`c.actor("admin", id)` **inside the transaction**, after authorization and before
writes. Identity's `authorize` and the admin repository's `admin_require_actor`
combine authorization/locking with actor attribution for authenticated mutations.
They accept authenticated server-side IDs; never take the audit actor from a request
body. Cross-context service calls and the outbox share that unit of work.

The unit of work rejects actor attribution outside a transaction.
Attribution uses transaction-local Postgres settings, so it expires on commit/rollback
and cannot leak through the connection pool. No actor means `actor_kind='system'`
and a NULL actor ID, including unauthenticated signup creation, password-recovery
requests, provider webhooks, and mail workers. After proving a single-use token,
subsequent changes are attributed to its user. Bootstrap creation is a system action.
`deleted_by_kind` distinguishes admin IDs from user IDs; no foreign key removes an
actor reference when its account is eventually purged.

Each history row contains:

| Column                   | Meaning                                                              |
| ------------------------ | -------------------------------------------------------------------- |
| `id`                     | Generated UUID of the history entry                                  |
| `record_id`              | UUID of the model record                                             |
| `operation`              | INSERT or UPDATE (soft deletion and restoration are updates)         |
| `changes`                | JSON object keyed by changed column, each containing `from` and `to` |
| `actor_id`, `actor_kind` | User/admin UUID or NULL/system                                       |
| `changed_at`             | Database wall-clock timestamp of the change                          |

Only changed fields are recorded; no-op updates do not create entries. The audit
write is in the same transaction, so any failure rolls back the business write too.
History has no model FK and survives a privileged purge. Pre-migration changes cannot
be reconstructed; existing rows begin accruing history on their next change.

Passwords, bearer-token hashes, identity-provider subjects, action-token payloads,
mail bodies (which contain action links), and checkout request details are redacted
in history. A secret change still appears, but its non-null values are `[redacted]`.
Add new secret columns to the model's trigger argument in a migration before writing
them. Do not store secrets under unclassified JSON fields. Email addresses/names and
other ordinary values remain auditable personal data; protect history access and
backups. The admin console exposes paginated user history; other companions are
available to trusted database operators and future context-specific admin screens.

## Operations and exceptional hard deletion

Soft-delete expired sessions, action tokens, invitations, rate-limit rows, and old
outbox rows as an explicit system maintenance transaction. There is no automatic
retention scheduler. Monitor history growth, especially rate-limit and worker records.
Do not silently reintroduce hard deletion as a cleanup job. Delivered mail is still
scrubbed in the current outbox record, and history never preserves its action links.

Hard deletion is an exceptional direct database operation requiring a backup,
reviewed scope, and a privileged operator. Application routes provide no purge option
or bypass setting. An operator must explicitly disable the relevant protection
trigger with table-owner privileges, perform targeted operations in a transaction,
and re-enable it before committing. Foreign keys use restrictive deletion, so dependent
records must be handled deliberately. History should ordinarily be preserved. Run
`SELECT validate_model_conventions()` afterward. Never run a deployment with disabled
triggers. There is intentionally no down migration that erases these histories;
rollback requires restoring a backup.

These triggers protect application DML and mistakes, not a malicious database owner:
a superuser/owner can alter tables, functions, triggers, or session settings. In
production, use trusted migration credentials for schema changes and a separate runtime
role with only SELECT/INSERT/UPDATE on models, SELECT/INSERT on companions, and sequence-free
UUID defaults. It must have no schema CREATE, table ownership, TRUNCATE, or DDL privileges.
The Docker startup migration mode uses an owner connection; separating migration
execution/roles is a deployment responsibility. The Lambda template separates the
migration connection from the API/mail connection, but runtime grants must still be
provisioned by the database operator. Audit exports to independent immutable
storage are a further extension when protection against database administrators is needed.
