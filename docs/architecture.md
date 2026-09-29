# Architecture and executable standards

AppShell is a modular monolith. Identity, organizations, and billing are bounded
contexts within one deployment and database. A context owns its use cases and
repository operations. Cross-context work goes through the owning context's
service API, with one transaction when the operation must be atomic.

## Rust dependency direction

```text
HTTP adapters -> context services -> pure domain rules
                         |
                         +-> infrastructure repositories and provider adapters

composition root: lib.rs / main.rs wires configuration, connections, and HTTP
```

- `apps/domain`: independent Rust crate containing credential validation,
  verification requirements, role policies, invitation eligibility, seat
  entitlements, and checkout eligibility. Functions receive facts and return
  decisions. No database, HTTP, clocks, randomness, logging, or environment access.
- `apps/api/src/http`: Axum extraction, cookies, middleware, response conversion,
  and OpenAPI annotations. Business endpoints call context services. Authentication
  middleware uses the identity service. This layer cannot use repositories or
  domain rules directly.
- `apps/api/src/contexts/{identity,organizations,billing}`: application services
  orchestrate authorization, domain decisions, transactions, and effects. Each
  context has a private implementation and a public facade in `mod.rs`. Services
  contain no SQL, Diesel types, Axum extractors, or HTTP client calls.
- `apps/api/src/infrastructure/repositories`: named, parameterized persistence
  operations. Method prefixes identify the owning context. Row mappings stay in
  infrastructure; serialized API models do not derive Diesel traits.
- `apps/api/src/infrastructure`: Postgres setup, migrations, password hashing,
  random tokens, Stripe HTTP/signature handling, and mail delivery. SQL is confined
  to repositories and database bootstrap/migration execution in `db.rs`.
- `models.rs` contains wire contracts, not entities with business behavior.
  Application errors are translated to HTTP responses in `http/error.rs`.

`UnitOfWork` provides a shared connection and explicit transaction closure.
Repository calls made inside it participate in that transaction, including
cross-context writes and the mail outbox. Never start a separate connection from
inside a transactional operation. `repositories::run` delegates to a blocking
worker; database and password-hashing work must remain off Tokio's async workers.
Provider HTTP calls happen outside database transactions.

Signup deliberately coordinates identity, initial organization membership, and a
free subscription in one transaction. Organization seat checks consume billing's
service API. Billing consumes organization authorization services. These explicit
collaborations do not require independently deployed services or separate databases.

This is a pragmatic DDD structure with concrete Postgres repositories. It does
not introduce generic CRUD repositories, one trait per function, CQRS, or an event
bus. Introduce a repository/provider port when another implementation or focused
service test actually needs it. Pure business rules already test without Postgres;
transaction and authorization behavior test against real Postgres.

## React dependency direction

```text
app/router -> pages -> feature public APIs -> validated HTTP client
                |              |
                +-> shared controls / shared libraries / configuration
```

- `router.tsx`, `main.tsx`, and `app/` compose routes, providers, and the workspace
  shell. They own application wiring.
- `pages/` compose screens and coordinate features. They can own form and display
  state, success navigation, and cache invalidation.
- `features/{identity,organizations,billing}/index.ts` is the public feature API.
  Endpoint paths, request types, response schemas, and query options live behind
  it. Features cannot import each other; compose cross-feature workflows in a page.
- `packages/ui` (`@appshell/ui`) owns reusable controls, theme behavior, and shared
  styles/tokens. It accepts data and callbacks through props and cannot depend on
  application code, routers, queries, or network APIs. Its public entry is `index.ts`.
- Web `components/` adapts those controls to application branding, TanStack Router,
  and form orchestration. Pages use shared controls instead of raw form elements
  or the library's private CSS classes. Add stories alongside each new control.
- `lib/api.ts` is the only network boundary. It handles cookies, cancellation,
  errors, and Zod validation. Generated types alone do not validate network data.
- `lib/query-client.ts` configures server-state caching. Feature query keys include
  organization IDs, and query functions forward cancellation signals.
- Keep server state in TanStack Query, local interaction state in React, and derive
  values instead of copying query results into component state. The shared workspace
  context holds the selected organization and session for presentation; the server
  remains the authorization authority.
- `config/brand.ts` and `packages/ui/src/tokens.css` define identity and appearance.
  Web `styles/tokens.css` is reserved for product-specific overrides. Keep
  accessible controls, keyboard behavior, and light/dark/system support.

React does not need to reproduce the backend's repository/service/domain layers.
Feature boundaries and explicit state ownership provide useful separation without
wrapping every hook or component in additional abstractions.

## Enforced checks

| Command                      | Enforcement                                                                                                       |
| ---------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| `npm run check`              | All formatting, frontend checks, architecture rules, and Rust lint checks                                         |
| `npm run format`             | Apply Prettier and rustfmt                                                                                        |
| `npm run format:check`       | Reject formatting drift without changing files                                                                    |
| `npm run check:web`          | Strict TypeScript, type-aware ESLint, Hooks rules, component exports, script linting; zero frontend lint warnings |
| `npm run check:architecture` | TypeScript import graph and Rust syntax boundary checks                                                           |
| `npm run check:rust`         | Workspace-wide Clippy on all targets, with warnings rejected                                                      |
| `npm test`                   | Frontend validation tests and architecture-checker negative fixtures                                              |
| `npm run test:rust`          | Domain, API, architectural, and Postgres integration tests                                                        |
| `npm run test:storybook`     | Shared component interactions, keyboard access, themes, responsive layout, and axe accessibility checks           |
| `npm run test:mail`          | Real API, outbox, SMTP, and Mailpit delivery of all email types                                                   |
| `npm run verify`             | Checks, unit/integration tests, app/Storybook builds, browser and local mail tests                                |

Rust workspace lints forbid unsafe code and flag production `unwrap`, `dbg!`,
`todo!`, and `unimplemented!`. Startup `expect` calls describe fatal configuration
failures. Test-only unwrap allowances permit concise fixtures. TypeScript retains
`strict`, unchecked-index checks, exact optional properties, and unused-code checks.
ESLint uses `strictTypeChecked`, exhaustive switch checks, and React Hooks rules.

The frontend architecture checker resolves imports with TypeScript's resolver,
including aliases, re-exports, dynamic literal imports, and type imports. It
rejects cycles, imports into feature internals, upward dependencies, page access
to the HTTP client, and direct network APIs outside the client. It also rejects raw form controls and
private component classes outside the shared UI package, package deep imports,
and application dependencies in the shared library. New source files
are checked even if they are omitted from `tsconfig`.

Rust tests parse source with `syn` and inspect tokens, including macro bodies.
They reject SQL outside persistence, infrastructure access from HTTP adapters,
framework dependencies/effects in the domain, direct provider HTTP from services,
and calls to another context's repository methods. Rust module privacy additionally
hides context implementation modules. Both checkers include intentionally invalid
fixtures so their rejection behavior is tested.

These are dependency and syntax checks, not proofs that all business logic is in
the right function. Review must still assess tenant scoping, transaction placement,
query-key completeness, semantic domain ownership, and accessibility. Do not add
blanket exclusions to make a violation pass; change the dependency or document and
test a narrowly justified rule change.

CI runs the same scripts, regenerates API contracts and rejects drift, then runs
browser tests. Configure the GitHub `Checks / check` status as a required branch
protection check to prevent merging failures; that repository setting is outside
these source-controlled scripts.

## Local prerequisites

The script runner uses local Cargo when available and otherwise runs Cargo in the
Compose API container. Set `APPSHELL_RUST_RUNNER=local` or `docker` to choose
explicitly. Docker checks require Docker Desktop and the development image;
`docker compose build api` builds it. Native Rust requires libpq, rustfmt, and
Clippy. Dependency lockfiles are committed; CI uses `npm ci` and locked Cargo builds.

For full validation, start `docker compose up -d db api`. Docker integration tests
use the Compose database and a disposable schema. Native integration tests require
`TEST_DATABASE_URL`; for the local Compose database use
`postgres://appshell:appshell@localhost:5433/appshell`. An explicitly provided URL
must be reachable from the selected runner. No script loads a local `.env` file.
Browser tests require the API running, plus `npx playwright install chromium` once.
Mailpit starts with the API; `test:mail` requires its HTTP endpoint on port 8025
and SMTP on 1025. It uses unique local accounts and leaves captured messages for
inspection. Storybook tests start their own local server on port 6006.
Restart the Compose API after Rust changes before testing browsers. The root
`test:e2e` command waits up to 60 seconds for `/ready` before starting Playwright.

## Adding a capability

1. Put deterministic business decisions and their tests in the domain crate.
2. Add context-owned repository methods with tenant-scoped parameters. Coordinate
   them in a context service and keep required locks/outbox writes in one transaction.
3. Expose the use case through the context facade and a small HTTP adapter. Update
   OpenAPI annotations and run `npm run generate:api` when the contract changes.
4. Add or update the Zod boundary schema and the feature API/query options. Export
   the supported surface through `index.ts`; consume it from a screen.
5. Run relevant tests and `npm run check`. Changes spanning both stacks require the
   full validation suite. Use a new migration for schema changes.

References: [Cargo workspace lints](https://doc.rust-lang.org/cargo/reference/workspaces.html#the-lints-table),
[typescript-eslint typed configurations](https://typescript-eslint.io/users/configs/),
and [React's state and component design guidance](https://react.dev/learn/thinking-in-react).
