use sqlx::{PgPool, Postgres, Transaction};

use super::{
    DictionaryDetailInput, DictionaryInput, DictionaryListQuery, SysDictionary,
    model::SysDictionaryDetailRow,
};

pub(super) async fn list(
    pool: &PgPool,
    query: DictionaryListQuery,
) -> Result<Vec<SysDictionary>, sqlx::Error> {
    let list = sqlx::query_as::<_, SysDictionary>(
        r#"
        select id, name, type as dict_type, status, "desc", parent_id
        from sys_dictionaries
        where ($1::text is null or name ilike '%' || $1 || '%')
        order by id desc
        "#,
    )
    .bind(query.name.as_deref())
    .fetch_all(pool)
    .await?;

    Ok(list)
}

pub(super) async fn insert(pool: &PgPool, payload: DictionaryInput) -> Result<(), sqlx::Error> {
    sqlx::query(
        "insert into sys_dictionaries (name, type, status, \"desc\", parent_id) values ($1, $2, $3, $4, $5)",
    )
    .bind(payload.name)
    .bind(payload.dict_type)
    .bind(payload.status)
    .bind(payload.desc)
    .bind(payload.parent_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub(super) async fn update(
    pool: &PgPool,
    id: i64,
    payload: DictionaryInput,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "update sys_dictionaries set name = $1, type = $2, status = $3, \"desc\" = $4, parent_id = $5 where id = $6",
    )
    .bind(payload.name)
    .bind(payload.dict_type)
    .bind(payload.status)
    .bind(payload.desc)
    .bind(payload.parent_id)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub(super) async fn find(pool: &PgPool, id: i64) -> Result<Option<SysDictionary>, sqlx::Error> {
    sqlx::query_as::<_, SysDictionary>(
        "select id, name, type as dict_type, status, \"desc\", parent_id from sys_dictionaries where id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub(super) async fn find_by_type(
    pool: &PgPool,
    dict_type: &str,
) -> Result<Option<SysDictionary>, sqlx::Error> {
    sqlx::query_as::<_, SysDictionary>(
        "select id, name, type as dict_type, status, \"desc\", parent_id from sys_dictionaries where type = $1",
    )
    .bind(dict_type)
    .fetch_optional(pool)
    .await
}

pub(super) async fn delete(pool: &PgPool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("delete from sys_dictionary_details where sys_dictionary_id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    sqlx::query("delete from sys_dictionaries where id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub(super) async fn insert_detail(
    pool: &PgPool,
    dictionary_id: i64,
    payload: DictionaryDetailInput,
    level: i32,
    path: String,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        insert into sys_dictionary_details
        (label, value, extend, status, sort, sys_dictionary_id, parent_id, level, path)
        values ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        "#,
    )
    .bind(payload.label)
    .bind(payload.value)
    .bind(payload.extend)
    .bind(payload.status)
    .bind(payload.sort)
    .bind(dictionary_id)
    .bind(payload.parent_id)
    .bind(level)
    .bind(path)
    .execute(pool)
    .await?;
    Ok(())
}

pub(super) async fn parent_is_in_subtree(
    tx: &mut Transaction<'_, Postgres>,
    dictionary_id: i64,
    detail_id: i64,
    parent_id: i64,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        with recursive subtree as (
            select id from sys_dictionary_details
            where sys_dictionary_id = $1 and id = $2
            union all
            select child.id from sys_dictionary_details child
            join subtree parent on child.parent_id = parent.id
            where child.sys_dictionary_id = $1
        )
        select exists(select 1 from subtree where id = $3)
        "#,
    )
    .bind(dictionary_id)
    .bind(detail_id)
    .bind(parent_id)
    .fetch_one(&mut **tx)
    .await
}

pub(super) async fn find_detail_level_path(
    tx: &mut Transaction<'_, Postgres>,
    dictionary_id: i64,
    detail_id: i64,
) -> Result<Option<(i32, String)>, sqlx::Error> {
    sqlx::query_as(
        "select level, path from sys_dictionary_details where sys_dictionary_id = $1 and id = $2",
    )
    .bind(dictionary_id)
    .bind(detail_id)
    .fetch_optional(&mut **tx)
    .await
}

pub(super) async fn find_detail_level_path_on_pool(
    pool: &PgPool,
    dictionary_id: i64,
    detail_id: i64,
) -> Result<Option<(i32, String)>, sqlx::Error> {
    sqlx::query_as(
        "select level, path from sys_dictionary_details where sys_dictionary_id = $1 and id = $2",
    )
    .bind(dictionary_id)
    .bind(detail_id)
    .fetch_optional(pool)
    .await
}

pub(super) async fn update_detail(
    tx: &mut Transaction<'_, Postgres>,
    dictionary_id: i64,
    detail_id: i64,
    payload: DictionaryDetailInput,
    level: i32,
    path: String,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        r#"
        update sys_dictionary_details
        set label = $1, value = $2, extend = $3, status = $4, sort = $5,
            sys_dictionary_id = $6, parent_id = $7, level = $8, path = $9
        where id = $10 and sys_dictionary_id = $11
        "#,
    )
    .bind(payload.label)
    .bind(payload.value)
    .bind(payload.extend)
    .bind(payload.status)
    .bind(payload.sort)
    .bind(dictionary_id)
    .bind(payload.parent_id)
    .bind(level)
    .bind(path)
    .bind(detail_id)
    .bind(dictionary_id)
    .execute(&mut **tx)
    .await?;
    Ok(result.rows_affected())
}

pub(super) async fn recalculate_descendant_paths(
    tx: &mut Transaction<'_, Postgres>,
    dictionary_id: i64,
    detail_id: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        with recursive descendants as (
            select id, level, path
            from sys_dictionary_details
            where sys_dictionary_id = $1 and id = $2
            union all
            select child.id,
                   parent.level + 1,
                   case when parent.path = '' then parent.id::text
                        else parent.path || ',' || parent.id::text end
            from sys_dictionary_details child
            join descendants parent on child.parent_id = parent.id
            where child.sys_dictionary_id = $1
        )
        update sys_dictionary_details detail
        set level = descendants.level, path = descendants.path
        from descendants
        where detail.id = descendants.id
          and detail.sys_dictionary_id = $1
          and detail.id <> $2
        "#,
    )
    .bind(dictionary_id)
    .bind(detail_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(super) async fn find_detail_row(
    pool: &PgPool,
    dictionary_id: i64,
    detail_id: i64,
) -> Result<Option<SysDictionaryDetailRow>, sqlx::Error> {
    sqlx::query_as::<_, SysDictionaryDetailRow>(
        r#"
        select id, label, value, extend, status, sort, sys_dictionary_id, parent_id, level, path
        from sys_dictionary_details where sys_dictionary_id = $1 and id = $2
        "#,
    )
    .bind(dictionary_id)
    .bind(detail_id)
    .fetch_optional(pool)
    .await
}

pub(super) async fn delete_detail_subtree(
    pool: &PgPool,
    dictionary_id: i64,
    detail_id: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        with recursive subtree as (
            select id
            from sys_dictionary_details
            where sys_dictionary_id = $1 and id = $2
            union all
            select child.id
            from sys_dictionary_details child
            join subtree parent on child.parent_id = parent.id
            where child.sys_dictionary_id = $1
        )
        delete from sys_dictionary_details
        where sys_dictionary_id = $1 and id in (select id from subtree)
        "#,
    )
    .bind(dictionary_id)
    .bind(detail_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub(super) async fn list_detail_rows(
    pool: &PgPool,
    sys_dictionary_id: i64,
) -> Result<Vec<SysDictionaryDetailRow>, sqlx::Error> {
    sqlx::query_as::<_, SysDictionaryDetailRow>(
        r#"
        select id, label, value, extend, status, sort, sys_dictionary_id, parent_id, level, path
        from sys_dictionary_details
        where sys_dictionary_id = $1
        order by sort asc, id asc
        "#,
    )
    .bind(sys_dictionary_id)
    .fetch_all(pool)
    .await
}

pub(super) async fn list_detail_rows_by_parent(
    pool: &PgPool,
    dictionary_id: i64,
    parent_id: i64,
) -> Result<Vec<SysDictionaryDetailRow>, sqlx::Error> {
    sqlx::query_as::<_, SysDictionaryDetailRow>(
        r#"
        select id, label, value, extend, status, sort, sys_dictionary_id, parent_id, level, path
        from sys_dictionary_details
        where sys_dictionary_id = $1 and parent_id = $2
        order by sort asc, id asc
        "#,
    )
    .bind(dictionary_id)
    .bind(parent_id)
    .fetch_all(pool)
    .await
}

pub(super) async fn list_detail_rows_by_ids(
    pool: &PgPool,
    dictionary_id: i64,
    ids: &[i64],
) -> Result<Vec<SysDictionaryDetailRow>, sqlx::Error> {
    sqlx::query_as::<_, SysDictionaryDetailRow>(
        r#"
        select id, label, value, extend, status, sort, sys_dictionary_id, parent_id, level, path
        from sys_dictionary_details
        where sys_dictionary_id = $1 and id = any($2)
        order by level asc, id asc
        "#,
    )
    .bind(dictionary_id)
    .bind(ids)
    .fetch_all(pool)
    .await
}
