//! `GuidanceStore` implementation for `PgStore`.

use crate::models::store::{GuidanceRule, GuidanceStore};
use async_trait::async_trait;
use sqlx::Row;

use crate::PgStore;

#[async_trait]
impl GuidanceStore for PgStore {
    async fn list_rules(&self, market_type: &str) -> anyhow::Result<Vec<GuidanceRule>> {
        let rows = sqlx::query(
            r#"SELECT id, market_type, rule_type, content, priority, enabled
               FROM guidance_rules
               WHERE market_type = $1 AND enabled = 1
               ORDER BY priority DESC"#,
        )
        .bind(market_type)
        .fetch_all(self.pool())
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| GuidanceRule {
                id: row.try_get("id").unwrap_or_default(),
                market_type: row.try_get("market_type").unwrap_or_default(),
                rule_type: row.try_get("rule_type").unwrap_or_default(),
                content: row.try_get("content").unwrap_or_default(),
                priority: row.try_get::<i32, _>("priority").unwrap_or_default(),
                enabled: row.try_get::<i32, _>("enabled").unwrap_or(1) != 0,
            })
            .collect())
    }

    async fn get_rule(&self, rule_id: &str) -> anyhow::Result<Option<GuidanceRule>> {
        let row = sqlx::query(
            r#"SELECT id, market_type, rule_type, content, priority, enabled
               FROM guidance_rules WHERE id = $1"#,
        )
        .bind(rule_id)
        .fetch_optional(self.pool())
        .await?;
        Ok(row.map(|row| GuidanceRule {
            id: row.try_get("id").unwrap_or_default(),
            market_type: row.try_get("market_type").unwrap_or_default(),
            rule_type: row.try_get("rule_type").unwrap_or_default(),
            content: row.try_get("content").unwrap_or_default(),
            priority: row.try_get::<i32, _>("priority").unwrap_or_default(),
            enabled: row.try_get::<i32, _>("enabled").unwrap_or(1) != 0,
        }))
    }

    async fn upsert_rule(&self, rule: &GuidanceRule) -> anyhow::Result<()> {
        sqlx::query(
            r#"INSERT INTO guidance_rules (id, market_type, rule_type, content, priority, enabled)
               VALUES ($1, $2, $3, $4, $5, $6)
               ON CONFLICT (id) DO UPDATE SET
                 market_type = EXCLUDED.market_type,
                 rule_type = EXCLUDED.rule_type,
                 content = EXCLUDED.content,
                 priority = EXCLUDED.priority,
                 enabled = EXCLUDED.enabled"#,
        )
        .bind(&rule.id)
        .bind(&rule.market_type)
        .bind(&rule.rule_type)
        .bind(&rule.content)
        .bind(rule.priority)
        .bind(if rule.enabled { 1i32 } else { 0i32 })
        .execute(self.pool())
        .await?;
        Ok(())
    }

    async fn delete_rule(&self, rule_id: &str) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM guidance_rules WHERE id = $1")
            .bind(rule_id)
            .execute(self.pool())
            .await?;
        Ok(())
    }
}
