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

    // Parse connection string
    // Supports two formats:
    // 1. oracle://user:password@host:port/service
    // 2. user/password@host:port/service
    let (username, password, connection_str) = if database_url.starts_with("oracle://") {
        // Remove oracle:// prefix
        let url_without_scheme = database_url.strip_prefix("oracle://")
            .ok_or("Failed to parse oracle:// URL")?;
        
        // Split by @ to separate credentials from connection string
        let parts: Vec<&str> = url_without_scheme.split('@').collect();
        if parts.len() != 2 {
            return Err(format!(
                "Invalid DATABASE_URL format. Expected: oracle://user:password@host:port/service. Got: {}",
                database_url
            ));
        }
        
        // Parse credentials (user:password)
        let cred_parts: Vec<&str> = parts[0].split(':').collect();
        if cred_parts.len() != 2 {
            return Err(format!(
                "Invalid credentials format. Expected: user:password. Got: {}",
                parts[0]
            ));
        }
        
        (cred_parts[0], cred_parts[1], parts[1])
    } else {
        // Legacy format: user/password@host:port/service
        let parts: Vec<&str> = database_url.split('@').collect();
        if parts.len() != 2 {
            return Err(format!(
                "Invalid DATABASE_URL format. Expected: user/password@host:port/service. Got: {}",
                database_url
            ));
        }
        
        let cred_parts: Vec<&str> = parts[0].split('/').collect();
        if cred_parts.len() != 2 {
            return Err(format!(
                "Invalid credentials format. Expected: user/password. Got: {}",
                parts[0]
            ));
        }
        
        (cred_parts[0], cred_parts[1], parts[1])
    };

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
