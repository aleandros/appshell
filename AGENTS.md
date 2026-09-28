# Repository guide

AppShell is a reusable SaaS starter. Keep changes generic, maintainable, and
consistent with the existing Rust + React architecture. Read `README.md` for
setup, deployment modes, and operational boundaries.

## Layout

- `apps/api`: Axum routes, Diesel/Postgres persistence, account flows, billing,
  mail delivery, migrations, and Rust tests.
- `apps/web`: strict TypeScript, React, TanStack Router/Query, Zod, and Tailwind.
- `apps/web/src/config/brand.ts` and `styles/tokens.css`: product identity and
  semantic light/dark design tokens.
- `openapi.json` and `apps/web/src/lib/api.generated.ts`: generated API contract.
- `compose.yaml`: local Postgres and API; `Dockerfile`: combined deployment;
  `.github/workflows/ci.yml`: required automated checks.

## Development

```sh
npm ci
docker compose up -d db api
npm run dev
```

The frontend uses port 5173, the API 8080, and local Postgres 5433. Development
email links appear in `docker compose logs -f api`. Restart the Compose API
after Rust changes to recompile it. Never commit real credentials, local env
files, database dumps, or development email logs.

## Conventions

- Preserve strict TypeScript settings. Validate untrusted data with Zod and
  use shared controls for loading, errors, validation, and empty states.
- Use semantic design tokens; check responsive layouts, keyboard access, and
  light/dark/system appearance when changing the UI.
- Keep blocking database and password-hashing work off Tokio's async workers.
- Scope tenant operations to an authorized organization. Membership, roles,
  verified-email gates, and subscription entitlements are enforced server-side.
- Preserve origin checks, secure session handling, single-use token semantics,
  transaction boundaries, and provider signature/idempotency checks.
- Add a new migration for schema changes; do not rewrite migrations already
  used by an installation. Keep migrations compatible with startup execution.
- After changing API DTOs/routes, run `npm run generate:api`, update the Zod
  boundary schemas, and commit both generated contract files. Do not hand-edit
  generated files. Keep `Cargo.lock` and `package-lock.json` committed.
- Update setup instructions and `.env.example` when configuration changes.

## Validation

Run the checks relevant to the change; use the full CI suite for changes that
span the stack. Documentation-only edits do not need application tests.

```sh
npm run check
npm test
npm run build
docker compose run --rm --no-deps api cargo fmt --all -- --check
docker compose run --rm --no-deps api cargo clippy --all-targets -- -D warnings
docker compose run --rm -e TEST_DATABASE_URL=postgres://appshell:appshell@db:5432/appshell api cargo test
npx playwright install chromium
npm run test:e2e
```

Browser tests require the API to be running and create throwaway local
accounts. Rust integration tests use a disposable Postgres schema. Exercise
authorization and cross-organization isolation when changing account/team
flows. Provider credentials are needed for live email or Stripe testing;
distinguish local test coverage from provider validation in your handoff.
