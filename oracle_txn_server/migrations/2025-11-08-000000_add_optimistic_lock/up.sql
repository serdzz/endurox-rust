-- Add Recver column for optimistic locking as first column
-- Oracle doesn't support positioning, so we'll add it and it will be last
-- But we can recreate the table to make it first

-- Create temporary table with Recver as first column
CREATE TABLE transactions_temp AS SELECT * FROM transactions;

-- Drop original table
DROP TABLE transactions CASCADE CONSTRAINTS;

-- Recreate table with Recver as first column
CREATE TABLE transactions (
    Recver NUMBER(10) DEFAULT 0 NOT NULL,
    id VARCHAR2(100) PRIMARY KEY,
    transaction_type VARCHAR2(50) NOT NULL,
    account VARCHAR2(100) NOT NULL,
    amount NUMBER(19,2) NOT NULL,
    currency VARCHAR2(10) NOT NULL,
    description VARCHAR2(500),
    status VARCHAR2(50) NOT NULL,
    message VARCHAR2(500),
    error_code VARCHAR2(50),
    error_message VARCHAR2(500),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL
);

-- Copy data back (Recver will default to 0)
INSERT INTO transactions (id, transaction_type, account, amount, currency, description, status, message, error_code, error_message, created_at, updated_at)
SELECT id, transaction_type, account, amount, currency, description, status, message, error_code, error_message, created_at, updated_at
FROM transactions_temp;

-- Drop temporary table
DROP TABLE transactions_temp;

-- Recreate indexes
CREATE INDEX idx_transactions_type ON transactions(transaction_type);
CREATE INDEX idx_transactions_status ON transactions(status);
CREATE INDEX idx_transactions_created ON transactions(created_at DESC);

-- Create trigger to auto-increment Recver on any UPDATE
CREATE OR REPLACE TRIGGER trg_transactions_optimistic_lock
BEFORE UPDATE ON transactions
FOR EACH ROW
BEGIN
    :NEW.Recver := :OLD.Recver + 1;
END;
