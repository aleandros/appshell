CREATE TABLE background_jobs (
 id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
 payload jsonb NOT NULL,
 created_at timestamptz NOT NULL DEFAULT now(),
 available_at timestamptz NOT NULL DEFAULT now(),
 attempts integer NOT NULL DEFAULT 0 CHECK (attempts >= 0),
 lease_version bigint NOT NULL DEFAULT 0,
 locked_until timestamptz,
 dispatched_until timestamptz,
 completed_at timestamptz,
 failed_at timestamptz,
 error_code text,
 CHECK (completed_at IS NULL OR failed_at IS NULL)
);
SELECT protect_model('background_jobs', ARRAY['payload']);
CREATE INDEX background_jobs_pending ON background_jobs(available_at, created_at)
 WHERE deleted_at IS NULL AND completed_at IS NULL AND failed_at IS NULL;
ALTER TABLE mail_outbox ADD COLUMN job_id uuid REFERENCES background_jobs(id);
CREATE INDEX mail_outbox_job ON mail_outbox(job_id) WHERE job_id IS NOT NULL;
