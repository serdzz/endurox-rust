-- Create transactions table for Oracle
CREATE TABLE transactions (
    id VARCHAR2(50) PRIMARY KEY,
    transaction_type VARCHAR2(50) NOT NULL,
    account VARCHAR2(50) NOT NULL,
    amount NUMBER NOT NULL,
    currency VARCHAR2(10) NOT NULL,
    description VARCHAR2(500),
    status VARCHAR2(20) NOT NULL,
    message VARCHAR2(500),
    error_code VARCHAR2(50),
    error_message VARCHAR2(500),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL
);

-- Create index on status for faster queries
CREATE INDEX idx_transactions_status ON transactions(status);

-- Create index on created_at for time-based queries
CREATE INDEX idx_transactions_created_at ON transactions(created_at);

-- Create trigger to update updated_at timestamp
CREATE OR REPLACE TRIGGER trg_transactions_updated_at
BEFORE UPDATE ON transactions
FOR EACH ROW
BEGIN
    :NEW.updated_at := CURRENT_TIMESTAMP;
END;
/
