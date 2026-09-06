use sqlx::PgPool;

#[derive(Clone)]
pub struct DictionaryService {
    pub(super) pool: PgPool,
}

impl DictionaryService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}
