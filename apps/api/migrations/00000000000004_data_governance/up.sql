-- Preserve existing installations; natural keys remain unique business keys.
ALTER TABLE users ADD COLUMN status text NOT NULL DEFAULT 'active' CHECK (status IN ('active','suspended'));
CREATE TABLE admin_accounts (
 id uuid PRIMARY KEY DEFAULT gen_random_uuid(), email text NOT NULL UNIQUE,
 name text NOT NULL, password_hash text NOT NULL,
 status text NOT NULL DEFAULT 'active' CHECK (status IN ('active','suspended')),
 created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE admin_sessions (
 id uuid PRIMARY KEY DEFAULT gen_random_uuid(), token_hash text NOT NULL UNIQUE,
 admin_id uuid NOT NULL REFERENCES admin_accounts,
 expires_at timestamptz NOT NULL, created_at timestamptz NOT NULL DEFAULT now()
);
ALTER TABLE billing_events RENAME COLUMN id TO provider_event_id;

-- Replace composite/natural primary keys with generated surrogate UUIDs.
DO $$
DECLARE t text; pk text;
BEGIN
 FOREACH t IN ARRAY ARRAY['identities','memberships','sessions','action_tokens','subscriptions','billing_events','rate_limits','billing_sync_locks','checkout_attempts'] LOOP
  SELECT conname INTO pk FROM pg_constraint WHERE conrelid=to_regclass(t) AND contype='p';
  EXECUTE format('ALTER TABLE %I DROP CONSTRAINT %I', t, pk);
  EXECUTE format('ALTER TABLE %I ADD COLUMN id uuid PRIMARY KEY DEFAULT gen_random_uuid()', t);
 END LOOP;
END $$;
ALTER TABLE identities ADD UNIQUE(provider,subject);
ALTER TABLE sessions ADD UNIQUE(token_hash);
ALTER TABLE action_tokens ADD UNIQUE(token_hash);
ALTER TABLE subscriptions ADD UNIQUE(organization_id);
ALTER TABLE billing_events ADD UNIQUE(provider_event_id);
ALTER TABLE rate_limits ADD UNIQUE(key);
ALTER TABLE billing_sync_locks ADD UNIQUE(provider_id);
ALTER TABLE checkout_attempts ADD UNIQUE(organization_id);
ALTER TABLE users ALTER COLUMN id SET DEFAULT gen_random_uuid();
ALTER TABLE organizations ALTER COLUMN id SET DEFAULT gen_random_uuid();
ALTER TABLE invitations ALTER COLUMN id SET DEFAULT gen_random_uuid();
ALTER TABLE mail_outbox ALTER COLUMN id SET DEFAULT gen_random_uuid();

-- No cascade hard deletes: even privileged maintenance must be explicit.
DO $$
DECLARE r record;
BEGIN
 FOR r IN SELECT conrelid::regclass AS t, conname, pg_get_constraintdef(oid) AS def
 FROM pg_constraint WHERE contype='f' AND connamespace=current_schema()::regnamespace AND confdeltype='c' LOOP
  EXECUTE format('ALTER TABLE %s DROP CONSTRAINT %I', r.t, r.conname);
  EXECUTE format('ALTER TABLE %s ADD CONSTRAINT %I %s', r.t, r.conname, replace(r.def,'ON DELETE CASCADE','ON DELETE RESTRICT'));
 END LOOP;
END $$;

CREATE FUNCTION reject_record_removal() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 RAISE EXCEPTION 'Hard deletion and history mutation are forbidden; use soft deletion. Privileged maintenance must explicitly disable the protection trigger.';
END $$;

CREATE FUNCTION prepare_model_write() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='UPDATE' AND NEW.id IS DISTINCT FROM OLD.id THEN
  RAISE EXCEPTION 'Model IDs are immutable';
 END IF;
 IF TG_OP='INSERT' THEN
  -- Ignore caller IDs: PostgreSQL is the sole authority for new database IDs.
  NEW.id := gen_random_uuid();
 END IF;
 IF NEW.deleted_at IS NOT NULL AND (TG_OP='INSERT' OR OLD.deleted_at IS NULL) THEN
  NEW.deleted_by := nullif(current_setting('app.actor_id',true),'')::uuid;
  NEW.deleted_by_kind := coalesce(nullif(current_setting('app.actor_kind',true),''),'system');
 ELSIF TG_OP='UPDATE' AND OLD.deleted_at IS NOT NULL AND NEW.deleted_at IS NOT NULL THEN
  NEW.deleted_at := OLD.deleted_at;
  NEW.deleted_by := OLD.deleted_by;
  NEW.deleted_by_kind := OLD.deleted_by_kind;
 ELSIF NEW.deleted_at IS NULL THEN
  NEW.deleted_by := NULL;
  NEW.deleted_by_kind := NULL;
 END IF;
 RETURN NEW;
END $$;

CREATE FUNCTION record_model_history() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE before_data jsonb; after_data jsonb; changes jsonb := '{}'::jsonb;
 k text; secret_columns text[] := TG_ARGV[0]::text[]; actor uuid; actor_kind text;
BEGIN
 before_data := CASE WHEN TG_OP='INSERT' THEN '{}'::jsonb ELSE to_jsonb(OLD) END;
 after_data := to_jsonb(NEW);
 FOR k IN SELECT jsonb_object_keys(after_data) LOOP
  IF before_data->k IS DISTINCT FROM after_data->k THEN
   changes := changes || jsonb_build_object(k, jsonb_build_object(
    'from', CASE WHEN k=ANY(secret_columns) AND before_data->k IS NOT NULL AND before_data->k <> 'null'::jsonb THEN '"[redacted]"'::jsonb ELSE before_data->k END,
    'to', CASE WHEN k=ANY(secret_columns) AND after_data->k <> 'null'::jsonb THEN '"[redacted]"'::jsonb ELSE after_data->k END));
  END IF;
 END LOOP;
 IF changes='{}'::jsonb THEN RETURN NEW; END IF;
 actor := nullif(current_setting('app.actor_id',true),'')::uuid;
 actor_kind := coalesce(nullif(current_setting('app.actor_kind',true),''),'system');
 EXECUTE format('INSERT INTO %I.%I(record_id,operation,changes,actor_id,actor_kind) VALUES($1,$2,$3,$4,$5)',TG_TABLE_SCHEMA,TG_TABLE_NAME||'_history')
 USING NEW.id, TG_OP, changes, actor, actor_kind;
 RETURN NEW;
END $$;

-- Every migration adding a model calls this once. History is append-only, without
-- a FK to its model, so it survives exceptional privileged purges.
CREATE FUNCTION protect_model(model regclass, secret_columns text[] DEFAULT '{}') RETURNS void LANGUAGE plpgsql AS $$
DECLARE model_name text; model_schema text; history_name text;
BEGIN
 SELECT relname,nspname INTO model_name,model_schema FROM pg_class JOIN pg_namespace ON pg_namespace.oid=relnamespace WHERE pg_class.oid=model;
 history_name := model_name||'_history';
 EXECUTE format('ALTER TABLE %s ADD COLUMN deleted_at timestamptz, ADD COLUMN deleted_by uuid, ADD COLUMN deleted_by_kind text CHECK (deleted_by_kind IN (''user'',''admin'',''system''))',model);
 EXECUTE format('CREATE TABLE %I.%I (id uuid PRIMARY KEY DEFAULT gen_random_uuid(), record_id uuid NOT NULL, operation text NOT NULL CHECK(operation IN (''INSERT'',''UPDATE'')), changes jsonb NOT NULL, actor_id uuid, actor_kind text NOT NULL CHECK(actor_kind IN (''user'',''admin'',''system'')), changed_at timestamptz NOT NULL DEFAULT clock_timestamp(), CHECK ((actor_kind=''system'') = (actor_id IS NULL)))',model_schema,history_name);
 EXECUTE format('CREATE INDEX ON %I.%I(record_id,changed_at)',model_schema,history_name);
 EXECUTE format('CREATE TRIGGER protect_history BEFORE UPDATE OR DELETE OR TRUNCATE ON %I.%I FOR EACH STATEMENT EXECUTE FUNCTION reject_record_removal()',model_schema,history_name);
 EXECUTE format('CREATE TRIGGER protect_removal BEFORE DELETE OR TRUNCATE ON %s FOR EACH STATEMENT EXECUTE FUNCTION reject_record_removal()',model);
 EXECUTE format('CREATE TRIGGER prepare_write BEFORE INSERT OR UPDATE ON %s FOR EACH ROW EXECUTE FUNCTION prepare_model_write()',model);
 EXECUTE format('CREATE TRIGGER record_history AFTER INSERT OR UPDATE ON %s FOR EACH ROW EXECUTE FUNCTION record_model_history(%L)',model,secret_columns::text);
END $$;
SELECT protect_model('users',ARRAY['password_hash']);
SELECT protect_model('admin_accounts',ARRAY['password_hash']);
SELECT protect_model('admin_sessions',ARRAY['token_hash']);
SELECT protect_model('identities',ARRAY['subject']);
SELECT protect_model('organizations');
SELECT protect_model('memberships');
SELECT protect_model('sessions',ARRAY['token_hash']);
SELECT protect_model('action_tokens',ARRAY['token_hash','payload']);
SELECT protect_model('invitations',ARRAY['token_hash']);
SELECT protect_model('subscriptions');
SELECT protect_model('billing_events');
SELECT protect_model('mail_outbox',ARRAY['body','html_body']);
SELECT protect_model('rate_limits',ARRAY['key']);
SELECT protect_model('billing_sync_locks',ARRAY['lease_id']);
SELECT protect_model('checkout_attempts',ARRAY['request_key','parameters']);

ALTER TABLE action_tokens DROP CONSTRAINT action_tokens_user_id_purpose_key;
CREATE UNIQUE INDEX action_tokens_active_purpose ON action_tokens(user_id,purpose) WHERE deleted_at IS NULL;
ALTER TABLE invitations DROP CONSTRAINT invitations_organization_id_email_key;
CREATE UNIQUE INDEX invitations_active_email ON invitations(organization_id,email) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX memberships_active_member ON memberships(organization_id,user_id) WHERE deleted_at IS NULL;

CREATE FUNCTION validate_model_conventions() RETURNS void LANGUAGE plpgsql AS $$
DECLARE r record;
BEGIN
 FOR r IN SELECT c.oid,c.relname FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace
 WHERE n.nspname=current_schema() AND c.relkind='r' AND c.relname <> '__diesel_schema_migrations' LOOP
  IF NOT EXISTS (SELECT 1 FROM pg_attribute a JOIN pg_attrdef d ON d.adrelid=a.attrelid AND d.adnum=a.attnum
   JOIN pg_constraint p ON p.conrelid=a.attrelid AND p.contype='p' AND p.conkey=ARRAY[a.attnum]
   WHERE a.attrelid=r.oid AND a.attname='id' AND a.atttypid='uuid'::regtype AND pg_get_expr(d.adbin,d.adrelid)='gen_random_uuid()') THEN
   RAISE EXCEPTION 'Table % needs a PostgreSQL-generated UUID primary key',r.relname;
  END IF;
  IF EXISTS (SELECT 1 FROM pg_trigger WHERE tgrelid=r.oid AND tgname='protect_history' AND tgfoid='reject_record_removal'::regproc AND tgtype=58 AND tgenabled='O') THEN
   IF NOT EXISTS (SELECT 1 FROM pg_class m JOIN pg_trigger t ON t.tgrelid=m.oid AND t.tgname='record_history' WHERE m.relnamespace=current_schema()::regnamespace AND m.relname||'_history'=r.relname) THEN
    RAISE EXCEPTION 'Orphan history table %',r.relname;
   END IF;
  ELSE
   IF (SELECT count(*) FROM pg_trigger WHERE tgrelid=r.oid AND tgenabled='O' AND (
      (tgname='protect_removal' AND tgfoid='reject_record_removal'::regproc AND tgtype=42) OR
      (tgname='prepare_write' AND tgfoid='prepare_model_write'::regproc AND tgtype=23) OR
      (tgname='record_history' AND tgfoid='record_model_history'::regproc AND tgtype=21))) <> 3
    OR (SELECT count(*) FROM pg_attribute WHERE attrelid=r.oid AND NOT attisdropped AND (
      (attname='deleted_at' AND atttypid='timestamptz'::regtype) OR
      (attname='deleted_by' AND atttypid='uuid'::regtype) OR
      (attname='deleted_by_kind' AND atttypid='text'::regtype))) <> 3
    OR to_regclass(format('%I.%I',current_schema(),r.relname||'_history')) IS NULL THEN
    RAISE EXCEPTION 'Table % must call protect_model()',r.relname;
   END IF;
  END IF;
 END LOOP;
END $$;
SELECT validate_model_conventions();
