-- Create CTP user and grant necessary privileges
-- This script runs automatically when Oracle container starts for the first time

-- Create user if not exists (works with pluggable databases)
DECLARE
  user_count NUMBER;
BEGIN
  SELECT COUNT(*) INTO user_count FROM all_users WHERE username = 'CTP';
  
  IF user_count = 0 THEN
    EXECUTE IMMEDIATE 'CREATE USER ctp IDENTIFIED BY ctp';
    EXECUTE IMMEDIATE 'GRANT CONNECT, RESOURCE TO ctp';
    EXECUTE IMMEDIATE 'GRANT CREATE SESSION TO ctp';
    EXECUTE IMMEDIATE 'GRANT CREATE TABLE TO ctp';
    EXECUTE IMMEDIATE 'GRANT CREATE VIEW TO ctp';
    EXECUTE IMMEDIATE 'GRANT CREATE SEQUENCE TO ctp';
    EXECUTE IMMEDIATE 'GRANT CREATE PROCEDURE TO ctp';
    EXECUTE IMMEDIATE 'GRANT UNLIMITED TABLESPACE TO ctp';
    DBMS_OUTPUT.PUT_LINE('User CTP created successfully');
  ELSE
    DBMS_OUTPUT.PUT_LINE('User CTP already exists');
  END IF;
END;
/

-- Create sample table for testing
CREATE TABLE ctp.test_table (
  id NUMBER PRIMARY KEY,
  name VARCHAR2(100),
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Create sequence for test_table
CREATE SEQUENCE ctp.test_table_seq START WITH 1 INCREMENT BY 1;

-- Insert sample data
INSERT INTO ctp.test_table (id, name) VALUES (ctp.test_table_seq.NEXTVAL, 'Test record 1');
INSERT INTO ctp.test_table (id, name) VALUES (ctp.test_table_seq.NEXTVAL, 'Test record 2');
COMMIT;

-- Create transactions table for oracle_txn_server
CREATE TABLE ctp.transactions (
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
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Create index on transaction type and status
CREATE INDEX idx_transactions_type ON ctp.transactions(transaction_type);
CREATE INDEX idx_transactions_status ON ctp.transactions(status);
CREATE INDEX idx_transactions_created ON ctp.transactions(created_at DESC);

COMMIT;

-- Verify
SELECT COUNT(*) as record_count FROM ctp.test_table;

EXIT;
