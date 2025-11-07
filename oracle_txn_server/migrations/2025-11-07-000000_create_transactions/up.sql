-- Create transactions table
CREATE TABLE transactions (
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

-- Create indexes for better query performance
CREATE INDEX idx_transactions_type ON transactions(transaction_type);
CREATE INDEX idx_transactions_status ON transactions(status);
CREATE INDEX idx_transactions_created ON transactions(created_at DESC);
