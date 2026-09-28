-- Serialize provider reads across API replicas without holding a DB connection during HTTP.
CREATE TABLE billing_sync_locks (
 provider_id text PRIMARY KEY, lease_id uuid NOT NULL, expires_at timestamptz NOT NULL
);
-- Reuse the same provider request until its Checkout session expires, across replicas/restarts.
CREATE TABLE checkout_attempts (
 organization_id uuid PRIMARY KEY REFERENCES organizations ON DELETE CASCADE,
 request_key text NOT NULL, parameters jsonb NOT NULL, expires_at timestamptz NOT NULL
);
