use sqlx::{MySqlPool, Row};
use glob::Pattern;

use crate::models::TableInfo;

pub async fn create_pool(conn: &str) -> Result<MySqlPool, sqlx::Error> {
    MySqlPool::connect(conn).await
}

/// Verifica se um nome de tabela deve ser excluído baseado em padrões glob
pub fn should_exclude_table(table_name: &str, exclude_patterns: &[String]) -> bool {
    for pattern_str in exclude_patterns {
        if let Ok(pattern) = Pattern::new(pattern_str) {
            if pattern.matches(table_name) {
                return true;
            }
        }
    }
    false
}

const SYSTEM_DATABASES: &[&str] = &[
    "sys",
    "information_schema",
    "mysql",
    "performance_schema",
];

pub async fn list_databases(pool: &MySqlPool) -> Result<Vec<String>, sqlx::Error> {
    let rows = sqlx::query("SHOW DATABASES").fetch_all(pool).await?;

    let mut dbs = Vec::new();

    for row in rows {
        let name: String = row.get(0);
        if !SYSTEM_DATABASES.contains(&name.as_str()) {
            dbs.push(name);
        }
    }

    Ok(dbs)
}

pub async fn list_tables_with_size(
    pool: &MySqlPool,
    database: &str,
) -> Result<Vec<TableInfo>, sqlx::Error> {
    let rows = sqlx::query(
        "
        SELECT
        table_name,
        table_type,
        CAST(IFNULL(data_length,0) + IFNULL(index_length,0) AS UNSIGNED) AS size
        FROM information_schema.tables
        WHERE table_schema = ?
        ORDER BY table_name ASC
        ",
    )
    .bind(database)
    .fetch_all(pool)
    .await?;

    let mut tables = Vec::new();

    for row in rows {
        let name: String = row.get("table_name");
        let table_type: String = row.get("table_type");
        let size: u64 = row.get("size");
        let is_view = table_type == "VIEW";

        tables.push(TableInfo { name, size, is_view });
    }

    Ok(tables)
}
