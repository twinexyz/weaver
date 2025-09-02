//! Database operations

/// Database operations
pub mod operations;

/// Database transaction
/// Transactions needs to be committed manually
pub mod transactions;

/// Run all database migrations at compile time
pub fn run_migrations() {
    // This will run all migrations in the migrations directory at compile time
    // The sqlx::migrate! macro ensures migrations are embedded and checked at
    // compile time
    sqlx::migrate!("./migrations");
}
