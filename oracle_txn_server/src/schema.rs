// SQL statements for transactions table

pub const CREATE_TRANSACTION: &str = r#"
    INSERT INTO transactions (
        id, transaction_type, account, amount, currency,
        description, status, message, error_code, error_message,
        created_at, updated_at
    ) VALUES (
        :1, :2, :3, :4, :5,
        :6, :7, :8, :9, :10,
        CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
    )
"#;

pub const GET_TRANSACTION: &str = r#"
    SELECT 
        id, transaction_type, account, amount, currency,
        description, status, message, error_code, error_message,
        created_at, updated_at
    FROM transactions
    WHERE id = :1
"#;

pub const LIST_TRANSACTIONS: &str = r#"
    SELECT 
        id, transaction_type, account, amount, currency,
        description, status, message, error_code, error_message,
        created_at, updated_at
    FROM transactions
    ORDER BY created_at DESC
    FETCH FIRST 100 ROWS ONLY
"#;
