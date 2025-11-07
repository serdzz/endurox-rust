use oracle::{Connection, pool::{Pool, PoolBuilder}};
use std::env;
use std::sync::Arc;

pub type DbPool = Arc<Pool>;

/// Initialize database connection pool
pub fn init_pool() -> Result<DbPool, String> {
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| {
            // Default Oracle connection string format:
            // username/password@hostname:port/service_name
            "endurox/endurox@localhost:1521/XEPDB1".to_string()
        });

    // Parse connection string (format: user/password@host:port/service)
    let parts: Vec<&str> = database_url.split('@').collect();
    if parts.len() != 2 {
        return Err("Invalid DATABASE_URL format. Expected: user/password@host:port/service".to_string());
    }

    let credentials = parts[0];
    let cred_parts: Vec<&str> = credentials.split('/').collect();
    if cred_parts.len() != 2 {
        return Err("Invalid credentials format. Expected: user/password".to_string());
    }

    let username = cred_parts[0];
    let password = cred_parts[1];
    let connection_str = parts[1];

    let pool = PoolBuilder::new(
        username,
        password,
        connection_str,
    )
    .max_connections(10)
    .build()
    .map_err(|e| format!("Failed to create pool: {}", e))?;
    
    Ok(Arc::new(pool))
}

/// Get a connection from the pool
pub fn get_connection(pool: &DbPool) -> Result<Connection, String> {
    pool.get()
        .map_err(|e| format!("Failed to get connection from pool: {}", e))
}
