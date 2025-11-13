use diesel::r2d2::{self, ConnectionManager};
use diesel::PgConnection;
use diesel_oci::OciConnection;
use std::env;

pub enum DbPool {
    Postgres(r2d2::Pool<ConnectionManager<PgConnection>>),
    Oracle(r2d2::Pool<ConnectionManager<OciConnection>>),
}

pub enum DbConnection {
    Postgres(r2d2::PooledConnection<ConnectionManager<PgConnection>>),
    Oracle(r2d2::PooledConnection<ConnectionManager<OciConnection>>),
}

/// Initialize Diesel database connection pool based on DATABASE_URL
pub fn init_pool() -> Result<DbPool, String> {
    let database_url = env::var("DATABASE_URL").map_err(|_| {
        "DATABASE_URL environment variable not set. \
         Examples:\n\
         - PostgreSQL: export DATABASE_URL='postgres://user:password@host:port/database'\n\
         - Oracle: export DATABASE_URL='oracle://user:password@host:port/service'"
            .to_string()
    })?;

    // Determine database type from URL scheme
    if database_url.starts_with("postgres://") || database_url.starts_with("postgresql://") {
        let manager = ConnectionManager::<PgConnection>::new(&database_url);
        let pool = r2d2::Pool::builder()
            .max_size(10)
            .build(manager)
            .map_err(|e| format!("Failed to create PostgreSQL connection pool: {}", e))?;
        Ok(DbPool::Postgres(pool))
    } else if database_url.starts_with("oracle://") {
        let manager = ConnectionManager::<OciConnection>::new(&database_url);
        let pool = r2d2::Pool::builder()
            .max_size(10)
            .build(manager)
            .map_err(|e| format!("Failed to create Oracle connection pool: {}", e))?;
        Ok(DbPool::Oracle(pool))
    } else {
        Err(format!(
            "Unsupported database URL scheme. Must start with 'postgres://', 'postgresql://', or 'oracle://'. Got: {}",
            database_url.split(':').next().unwrap_or("unknown")
        ))
    }
}

/// Get a connection from the pool
pub fn get_connection(pool: &DbPool) -> Result<DbConnection, String> {
    match pool {
        DbPool::Postgres(pg_pool) => pg_pool
            .get()
            .map(DbConnection::Postgres)
            .map_err(|e| format!("Failed to get PostgreSQL connection from pool: {}", e)),
        DbPool::Oracle(oci_pool) => oci_pool
            .get()
            .map(DbConnection::Oracle)
            .map_err(|e| format!("Failed to get Oracle connection from pool: {}", e)),
    }
}
