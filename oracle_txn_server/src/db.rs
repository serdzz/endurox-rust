use diesel::r2d2::{self, ConnectionManager};
use diesel_oci::OciConnection;
use std::env;

pub type DbPool = r2d2::Pool<ConnectionManager<OciConnection>>;
pub type DbConnection = r2d2::PooledConnection<ConnectionManager<OciConnection>>;

/// Initialize Diesel database connection pool
pub fn init_pool() -> Result<DbPool, String> {
    let database_url = env::var("DATABASE_URL").map_err(|_| {
        "DATABASE_URL environment variable not set. \
         Example: export DATABASE_URL='oracle://user:password@host:port/service'"
            .to_string()
    })?;

    let manager = ConnectionManager::<OciConnection>::new(&database_url);
    
    r2d2::Pool::builder()
        .max_size(10)
        .build(manager)
        .map_err(|e| format!("Failed to create connection pool: {}", e))
}

/// Get a connection from the pool
pub fn get_connection(pool: &DbPool) -> Result<DbConnection, String> {
    pool.get()
        .map_err(|e| format!("Failed to get connection from pool: {}", e))
}
