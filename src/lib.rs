pub mod config;
pub mod db;
pub mod error;
pub mod models;
pub mod render;
mod web;

use sqlx::SqlitePool;
#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
}
pub use db::{create_pool, initialize};
pub use web::router;
