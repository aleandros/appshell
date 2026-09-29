-- Audit retention and changed primary keys make this intentionally irreversible.
DO $$ BEGIN RAISE EXCEPTION 'Restore a backup to roll back data governance'; END $$;
