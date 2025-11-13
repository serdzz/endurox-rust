-- Drop trigger
DROP TRIGGER trg_transactions_updated_at;

-- Drop indexes
DROP INDEX idx_transactions_created;
DROP INDEX idx_transactions_status;
DROP INDEX idx_transactions_type;

-- Drop transactions table
DROP TABLE transactions;
