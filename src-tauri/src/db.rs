use sqlx::{MySqlPool, Row};

use crate::models::{DatabaseInfo, TableInfo};

pub async fn create_pool(conn: &str) -> Result<MySqlPool, sqlx::Error> {
    MySqlPool::connect(conn).await
}

const SYSTEM_DATABASES: &[&str] = &[
    "sys",
    "information_schema",
    "mysql",
    "performance_schema",
];

pub async fn list_databases(pool: &MySqlPool) -> Result<Vec<DatabaseInfo>, sqlx::Error> {
    let rows = sqlx::query(
        "
        SELECT
          s.schema_name AS name,
          CAST(
            IFNULL(SUM(t.data_length), 0) + IFNULL(SUM(t.index_length), 0)
            AS UNSIGNED
          ) AS size
        FROM information_schema.schemata s
        LEFT JOIN information_schema.tables t
          ON t.table_schema = s.schema_name
        WHERE s.schema_name NOT IN (
          'sys', 'information_schema', 'mysql', 'performance_schema'
        )
        GROUP BY s.schema_name
        ORDER BY s.schema_name ASC
        ",
    )
    .fetch_all(pool)
    .await?;

    let mut dbs = Vec::new();

    for row in rows {
        let name: String = row.get("name");
        if SYSTEM_DATABASES.contains(&name.as_str()) {
            continue;
        }
        let size: u64 = row.get("size");
        dbs.push(DatabaseInfo { name, size });
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

        let size: u64 = row.get("size");

        tables.push(TableInfo { name, size });
    }

    Ok(tables)
}
