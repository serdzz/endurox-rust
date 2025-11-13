-- Drop trigger
DROP TRIGGER IF EXISTS trg_transactions_updated_at ON transactions;

-- Drop function
DROP FUNCTION IF EXISTS update_updated_at_column();

-- Drop indexes
DROP INDEX IF EXISTS idx_transactions_created;
DROP INDEX IF EXISTS idx_transactions_status;
DROP INDEX IF EXISTS idx_transactions_type;

-- Drop transactions table
DROP TABLE IF EXISTS transactions;
