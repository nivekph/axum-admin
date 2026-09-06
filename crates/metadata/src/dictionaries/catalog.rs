use super::{
    DictionaryError, DictionaryInput, DictionaryListQuery, DictionaryService,
    DictionaryWithDetails, SysDictionary, repository, tree,
};

impl DictionaryService {
    pub async fn list(
        &self,
        query: DictionaryListQuery,
    ) -> Result<Vec<SysDictionary>, DictionaryError> {
        Ok(repository::list(&self.pool, query).await?)
    }

    pub async fn create(&self, payload: DictionaryInput) -> Result<(), DictionaryError> {
        Ok(repository::insert(&self.pool, payload).await?)
    }

    pub async fn update(&self, id: i64, payload: DictionaryInput) -> Result<(), DictionaryError> {
        Ok(repository::update(&self.pool, id, payload).await?)
    }

    pub async fn find(
        &self,
        id: Option<i64>,
        kind: Option<String>,
    ) -> Result<Option<DictionaryWithDetails>, DictionaryError> {
        let dictionary = if let Some(id) = id {
            repository::find(&self.pool, id).await?
        } else if let Some(dict_type) = kind {
            repository::find_by_type(&self.pool, &dict_type).await?
        } else {
            None
        };

        if let Some(dictionary) = dictionary {
            let details = tree::tree_for_dictionary(&self.pool, dictionary.id).await?;
            return Ok(Some(DictionaryWithDetails {
                dictionary,
                details,
            }));
        }

        Ok(None)
    }

    pub async fn delete(&self, id: i64) -> Result<(), DictionaryError> {
        Ok(repository::delete(&self.pool, id).await?)
    }

    pub async fn export(&self, id: i64) -> Result<Option<DictionaryWithDetails>, DictionaryError> {
        let Some(dictionary) = repository::find(&self.pool, id).await? else {
            return Ok(None);
        };
        let details = tree::tree_for_dictionary(&self.pool, id).await?;
        Ok(Some(DictionaryWithDetails {
            dictionary,
            details,
        }))
    }

    pub async fn import(&self, payload: DictionaryInput) -> Result<(), DictionaryError> {
        Ok(repository::insert(&self.pool, payload).await?)
    }
}
