-- Add migration script here
BEGIN;
-- Backfill `status` for historical data
UPDATE subscriptions
SET status = 'confirmed'
WHERE status IS NULL;
-- Make `status` not nullable
ALTER TABLE subscriptions
ALTER COLUMN status
SET NOT NULL;
COMMIT;