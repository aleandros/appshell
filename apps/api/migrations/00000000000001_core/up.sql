CREATE TABLE users (
 id uuid PRIMARY KEY, email text NOT NULL UNIQUE, name text NOT NULL,
 password_hash text, email_verified_at timestamptz, created_at timestamptz NOT NULL DEFAULT now()
);
-- External identities are deliberately separate from the user: add OIDC without changing tenancy.
CREATE TABLE identities (
 provider text NOT NULL, subject text NOT NULL, user_id uuid NOT NULL REFERENCES users ON DELETE CASCADE,
 PRIMARY KEY (provider, subject), UNIQUE (user_id, provider)
);
CREATE TABLE organizations (id uuid PRIMARY KEY, name text NOT NULL, created_at timestamptz NOT NULL DEFAULT now());
CREATE TABLE memberships (
 organization_id uuid NOT NULL REFERENCES organizations ON DELETE CASCADE,
 user_id uuid NOT NULL REFERENCES users ON DELETE CASCADE,
 role text NOT NULL CHECK (role IN ('owner','admin','member')),
 PRIMARY KEY (organization_id, user_id)
);
CREATE INDEX memberships_user ON memberships(user_id);
CREATE TABLE sessions (
 token_hash text PRIMARY KEY, user_id uuid NOT NULL REFERENCES users ON DELETE CASCADE,
 expires_at timestamptz NOT NULL, created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX sessions_user ON sessions(user_id);
CREATE TABLE action_tokens (
 token_hash text PRIMARY KEY, user_id uuid NOT NULL REFERENCES users ON DELETE CASCADE,
 purpose text NOT NULL CHECK (purpose IN ('verify','reset','email')),
 payload text, expires_at timestamptz NOT NULL, UNIQUE(user_id, purpose)
);
CREATE TABLE invitations (
 id uuid PRIMARY KEY, organization_id uuid NOT NULL REFERENCES organizations ON DELETE CASCADE,
 email text NOT NULL, role text NOT NULL CHECK (role IN ('admin','member')),
 token_hash text NOT NULL UNIQUE, expires_at timestamptz NOT NULL,
 UNIQUE (organization_id, email)
);
CREATE TABLE subscriptions (
 organization_id uuid PRIMARY KEY REFERENCES organizations ON DELETE CASCADE,
 plan text NOT NULL DEFAULT 'free' CHECK(plan IN ('free','pro')),
 status text NOT NULL DEFAULT 'active' CHECK(status IN ('active','trialing','past_due','canceled','unpaid','incomplete','incomplete_expired','paused')),
 provider_id text UNIQUE, customer_id text, current_period_end timestamptz,
 updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE billing_events (id text PRIMARY KEY, created_at timestamptz NOT NULL DEFAULT now());
CREATE TABLE mail_outbox (
 id uuid PRIMARY KEY, recipient text NOT NULL, subject text NOT NULL, body text NOT NULL,
 attempts integer NOT NULL DEFAULT 0, available_at timestamptz NOT NULL DEFAULT now(),
 sent_at timestamptz, created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX mail_outbox_pending ON mail_outbox(available_at) WHERE sent_at IS NULL;
CREATE TABLE rate_limits (key text PRIMARY KEY, hits integer NOT NULL, expires_at timestamptz NOT NULL);
