-- Nullable preserves jobs queued by older versions; workers render a fallback for them.
ALTER TABLE mail_outbox ADD COLUMN html_body text;
