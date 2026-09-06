use std::collections::HashMap;

use sqlx::PgPool;

use super::{
    DictionaryDetailInput, DictionaryError, DictionaryService, SysDictionaryDetail,
    model::SysDictionaryDetailRow, repository,
};

impl DictionaryService {
    pub async fn create_detail(
        &self,
        dictionary_id: i64,
        payload: DictionaryDetailInput,
    ) -> Result<(), DictionaryError> {
        ensure_dictionary_exists(&self.pool, dictionary_id).await?;
        let (level, path) = match payload.parent_id {
            Some(parent_id) => level_and_path_for_parent(&self.pool, dictionary_id, parent_id)
                .await?
                .ok_or(DictionaryError::DetailNotFound {
                    dictionary_id,
                    detail_id: parent_id,
                })?,
            None => (0, String::new()),
        };
        Ok(repository::insert_detail(
            &self.pool,
            dictionary_id,
            payload.label,
            payload.value,
            payload.extend,
            payload.status,
            payload.sort,
            payload.parent_id,
            level,
            path,
        )
        .await?)
    }

    pub async fn update_detail(
        &self,
        dictionary_id: i64,
        detail_id: i64,
        payload: DictionaryDetailInput,
    ) -> Result<(), DictionaryError> {
        let mut tx = self.pool.begin().await?;
        if let Some(parent_id) = payload.parent_id {
            let invalid_parent =
                repository::parent_is_in_subtree(&mut tx, dictionary_id, detail_id, parent_id)
                    .await?;
            if invalid_parent {
                return Err(DictionaryError::InvalidParent {
                    dictionary_id,
                    detail_id,
                    parent_id,
                });
            }
        }
        let (level, path) = match payload.parent_id {
            Some(parent_id) => {
                let parent_info =
                    repository::find_detail_level_path(&mut tx, dictionary_id, parent_id).await?;
                match parent_info {
                    Some((level, path)) => Ok((level + 1, child_path(&path, parent_id))),
                    None => Err(DictionaryError::DetailNotFound {
                        dictionary_id,
                        detail_id: parent_id,
                    }),
                }
            }
            None => Ok((0, String::new())),
        }?;
        let rows_affected = repository::update_detail(
            &mut tx,
            dictionary_id,
            detail_id,
            payload.label,
            payload.value,
            payload.extend,
            payload.status,
            payload.sort,
            payload.parent_id,
            level,
            path,
        )
        .await?;
        if rows_affected == 0 {
            return Err(DictionaryError::DetailNotFound {
                dictionary_id,
                detail_id,
            });
        }
        repository::recalculate_descendant_paths(&mut tx, dictionary_id, detail_id).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn find_detail(
        &self,
        dictionary_id: i64,
        detail_id: i64,
    ) -> Result<SysDictionaryDetail, DictionaryError> {
        repository::find_detail_row(&self.pool, dictionary_id, detail_id)
            .await?
            .map(detail_from_row)
            .ok_or(DictionaryError::DetailNotFound {
                dictionary_id,
                detail_id,
            })
    }

    pub async fn delete_detail(
        &self,
        dictionary_id: i64,
        detail_id: i64,
    ) -> Result<(), DictionaryError> {
        self.find_detail(dictionary_id, detail_id).await?;
        Ok(repository::delete_detail_subtree(&self.pool, dictionary_id, detail_id).await?)
    }

    pub async fn tree_by_dictionary(
        &self,
        id: i64,
    ) -> Result<Vec<SysDictionaryDetail>, DictionaryError> {
        Ok(tree_for_dictionary(&self.pool, id).await?)
    }

    pub async fn tree_by_type(
        &self,
        kind: &str,
    ) -> Result<Vec<SysDictionaryDetail>, DictionaryError> {
        if let Some(dictionary) = repository::find_by_type(&self.pool, kind).await? {
            return Ok(tree_for_dictionary(&self.pool, dictionary.id).await?);
        }
        Ok(Vec::new())
    }

    pub async fn details_by_parent(
        &self,
        dictionary_id: i64,
        parent_id: i64,
    ) -> Result<Vec<SysDictionaryDetail>, DictionaryError> {
        let rows =
            repository::list_detail_rows_by_parent(&self.pool, dictionary_id, parent_id).await?;
        Ok(rows.into_iter().map(detail_from_row).collect())
    }

    pub async fn detail_path(
        &self,
        dictionary_id: i64,
        detail_id: i64,
    ) -> Result<Vec<SysDictionaryDetail>, DictionaryError> {
        let item = self.find_detail(dictionary_id, detail_id).await?;
        if item.path.is_empty() {
            return Ok(vec![item]);
        }
        let mut ids = item
            .path
            .split(',')
            .filter_map(|part| part.parse::<i64>().ok())
            .collect::<Vec<_>>();
        ids.push(item.id);
        let rows = repository::list_detail_rows_by_ids(&self.pool, dictionary_id, &ids).await?;
        Ok(rows.into_iter().map(detail_from_row).collect())
    }
}

pub(super) async fn tree_for_dictionary(
    pool: &PgPool,
    sys_dictionary_id: i64,
) -> Result<Vec<SysDictionaryDetail>, sqlx::Error> {
    let rows = repository::list_detail_rows(pool, sys_dictionary_id).await?;
    let mut rows_by_parent = HashMap::<Option<i64>, Vec<_>>::new();
    for row in rows {
        rows_by_parent.entry(row.parent_id).or_default().push(row);
    }
    Ok(build_detail_tree(&mut rows_by_parent, None))
}

async fn ensure_dictionary_exists(
    pool: &PgPool,
    dictionary_id: i64,
) -> Result<(), DictionaryError> {
    if repository::find(pool, dictionary_id).await?.is_none() {
        return Err(DictionaryError::DictionaryNotFound { dictionary_id });
    }
    Ok(())
}

async fn level_and_path_for_parent(
    pool: &PgPool,
    dictionary_id: i64,
    parent_id: i64,
) -> Result<Option<(i32, String)>, sqlx::Error> {
    let parent = repository::find_detail_level_path_on_pool(pool, dictionary_id, parent_id).await?;
    Ok(parent.map(|(level, path)| (level + 1, child_path(&path, parent_id))))
}

fn child_path(parent_path: &str, parent_id: i64) -> String {
    if parent_path.is_empty() {
        parent_id.to_string()
    } else {
        format!("{parent_path},{parent_id}")
    }
}

fn build_detail_tree(
    rows_by_parent: &mut HashMap<Option<i64>, Vec<SysDictionaryDetailRow>>,
    parent_id: Option<i64>,
) -> Vec<SysDictionaryDetail> {
    rows_by_parent
        .remove(&parent_id)
        .unwrap_or_default()
        .into_iter()
        .map(|row| {
            let id = row.id;
            let mut item = detail_from_row(row);
            item.children = build_detail_tree(rows_by_parent, Some(id));
            item
        })
        .collect()
}

fn detail_from_row(row: SysDictionaryDetailRow) -> SysDictionaryDetail {
    SysDictionaryDetail {
        id: row.id,
        label: row.label,
        value: row.value,
        extend: row.extend,
        status: row.status,
        sort: row.sort,
        sys_dictionary_id: row.sys_dictionary_id,
        parent_id: row.parent_id,
        level: row.level,
        path: row.path,
        children: Vec::new(),
    }
}
