use std::collections::BTreeMap;

use serde::Serialize;
use sqlx::{FromRow, PgConnection, PgPool};
use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{AuditError, AuditEvent, AuditEventView, AuditQuery};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuditDailyStat {
    pub date: String,
    pub logins: i64,
    pub ips: i64,
    pub login_failures: i64,
    pub access_denials: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuditStats {
    pub days: i64,
    pub event_count: i64,
    pub today_logins: i64,
    pub today_ips: i64,
    pub daily: Vec<AuditDailyStat>,
}

#[derive(Clone)]
pub struct AuditService {
    pool: PgPool,
}

impl AuditService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn record(&self, event: AuditEvent) -> Result<(), AuditError> {
        let mut connection = self.pool.acquire().await?;
        Self::record_in(&mut connection, event).await
    }

    pub async fn record_best_effort(&self, event: AuditEvent) {
        let action = event.action.to_string();
        let resource_type = event.resource.resource_type();
        let resource_id = event.resource.resource_id();
        let req_id = event.req_id.clone();
        if let Err(error) = self.record(event).await {
            tracing::error!(
                action,
                resource_type,
                resource_id,
                req_id,
                error = ?error,
                "HIGH PRIORITY: committed operation has no audit event"
            );
        }
    }

    pub(crate) async fn record_in(
        conn: &mut PgConnection,
        event: AuditEvent,
    ) -> Result<(), AuditError> {
        let action = event.action.to_string();
        let result = event.result.to_string();
        let reason_code = event.reason_code.map(|code| code.to_string());
        let changes = serde_json::to_value(event.changes)?;
        sqlx::query(
            r#"
            insert into sys_audit_events (
                req_id, actor_id, actor_label, action, resource_type, resource_id,
                result, reason_code, source_ip, user_agent, changes
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#,
        )
        .bind(event.req_id)
        .bind(event.actor.id)
        .bind(event.actor.label)
        .bind(action)
        .bind(event.resource.resource_type())
        .bind(event.resource.resource_id())
        .bind(result)
        .bind(reason_code)
        .bind(event.source.ip)
        .bind(event.source.user_agent)
        .bind(changes)
        .execute(conn)
        .await?;
        Ok(())
    }

    pub async fn list(
        &self,
        query: AuditQuery,
    ) -> Result<(Vec<AuditEventView>, i64, i64, i64), AuditError> {
        let started_at = parse_time(query.started_at.as_deref())?;
        let ended_at = parse_time(query.ended_at.as_deref())?;
        let page = query.page.max(1);
        let page_size = query.page_size.max(1);
        let offset = (page - 1) * page_size;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            select count(*) from sys_audit_events
            where ($1::text is null or req_id ilike '%' || $1 || '%')
              and ($2::text is null or actor_label ilike '%' || $2 || '%' or actor_id::text = $2)
              and ($3::text is null or action = $3)
              and ($4::text is null or resource_type = $4)
              and ($5::text is null or resource_id = $5)
              and ($6::text is null or result = $6)
              and ($7::timestamptz is null or created_at >= $7)
              and ($8::timestamptz is null or created_at <= $8)
            "#,
        )
        .bind(query.req_id.as_deref())
        .bind(query.actor.as_deref())
        .bind(query.action.as_deref())
        .bind(query.resource_type.as_deref())
        .bind(query.resource_id.as_deref())
        .bind(query.result.as_deref())
        .bind(started_at)
        .bind(ended_at)
        .fetch_one(&self.pool)
        .await?;

        let events = sqlx::query_as::<_, AuditEventView>(
            r#"
            select
                id, req_id, actor_id, actor_label, action, resource_type, resource_id, result,
                reason_code, source_ip, user_agent, changes,
                to_char(created_at at time zone 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as created_at
            from sys_audit_events
            where ($1::text is null or req_id ilike '%' || $1 || '%')
              and ($2::text is null or actor_label ilike '%' || $2 || '%' or actor_id::text = $2)
              and ($3::text is null or action = $3)
              and ($4::text is null or resource_type = $4)
              and ($5::text is null or resource_id = $5)
              and ($6::text is null or result = $6)
              and ($7::timestamptz is null or created_at >= $7)
              and ($8::timestamptz is null or created_at <= $8)
            order by id desc
            limit $9 offset $10
            "#,
        )
        .bind(query.req_id.as_deref())
        .bind(query.actor.as_deref())
        .bind(query.action.as_deref())
        .bind(query.resource_type.as_deref())
        .bind(query.resource_id.as_deref())
        .bind(query.result.as_deref())
        .bind(started_at)
        .bind(ended_at)
        .bind(page_size)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok((events, total, page, page_size))
    }

    pub async fn find(&self, id: i64) -> Result<Option<AuditEventView>, AuditError> {
        Ok(sqlx::query_as::<_, AuditEventView>(
            r#"
            select
                id, req_id, actor_id, actor_label, action, resource_type, resource_id, result,
                reason_code, source_ip, user_agent, changes,
                to_char(created_at at time zone 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as created_at
            from sys_audit_events
            where id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn stats(&self, days: i64) -> Result<AuditStats, AuditError> {
        let days = days.clamp(1, 90);
        let today = OffsetDateTime::now_utc().date();
        let start_date = today - Duration::days(days - 1);
        let end_date = today + Duration::days(1);
        let started_at = start_date
            .with_hms(0, 0, 0)
            .expect("UTC midnight is a valid time")
            .assume_utc();
        let ended_at = end_date
            .with_hms(0, 0, 0)
            .expect("UTC midnight is a valid time")
            .assume_utc();

        #[derive(FromRow)]
        struct DailyRow {
            day: String,
            logins: i64,
            ips: i64,
            login_failures: i64,
            access_denials: i64,
        }

        let event_count = sqlx::query_scalar::<_, i64>("select count(*) from sys_audit_events")
            .fetch_one(&self.pool)
            .await?;

        let rows = sqlx::query_as::<_, DailyRow>(
            r#"
            select
                to_char((created_at at time zone 'UTC')::date, 'YYYY-MM-DD') as day,
                count(*) filter (
                    where action = 'auth.login' and result = 'succeeded'
                )::bigint as logins,
                count(distinct nullif(source_ip, '')) filter (
                    where action = 'auth.login' and result = 'succeeded'
                )::bigint as ips,
                count(*) filter (
                    where action = 'auth.login' and result in ('failed', 'denied')
                )::bigint as login_failures,
                count(*) filter (
                    where action = 'auth.access_denied' and result = 'denied'
                )::bigint as access_denials
            from sys_audit_events
            where created_at >= $1 and created_at < $2
            group by 1
            order by 1
            "#,
        )
        .bind(started_at)
        .bind(ended_at)
        .fetch_all(&self.pool)
        .await?;

        let by_day = rows
            .into_iter()
            .map(|row| (row.day.clone(), row))
            .collect::<BTreeMap<_, _>>();

        let mut daily = Vec::with_capacity(days as usize);
        for offset in (0..days).rev() {
            let date = today - Duration::days(offset);
            let key = date.to_string();
            let row = by_day.get(&key);
            daily.push(AuditDailyStat {
                date: key,
                logins: row.map_or(0, |row| row.logins),
                ips: row.map_or(0, |row| row.ips),
                login_failures: row.map_or(0, |row| row.login_failures),
                access_denials: row.map_or(0, |row| row.access_denials),
            });
        }

        let today_logins = daily.last().map_or(0, |row| row.logins);
        let today_ips = daily.last().map_or(0, |row| row.ips);

        Ok(AuditStats {
            days,
            event_count,
            today_logins,
            today_ips,
            daily,
        })
    }
}

fn parse_time(value: Option<&str>) -> Result<Option<OffsetDateTime>, AuditError> {
    value
        .map(|value| OffsetDateTime::parse(value, &Rfc3339).map_err(AuditError::InvalidTimeRange))
        .transpose()
}
