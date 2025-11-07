-- Drop trigger
DROP TRIGGER trg_transactions_updated_at;

-- Drop indexes
DROP INDEX idx_transactions_created_at;
DROP INDEX idx_transactions_status;

-- Drop table
DROP TABLE transactions;
