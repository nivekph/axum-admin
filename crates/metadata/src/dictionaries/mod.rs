mod catalog;
mod error;
mod model;
mod repository;
mod request;
mod tree;

pub use error::DictionaryError;
pub use model::*;
pub use request::*;
use sqlx::PgPool;

#[derive(Clone)]
pub struct DictionaryService {
    pool: PgPool,
}

impl DictionaryService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}
