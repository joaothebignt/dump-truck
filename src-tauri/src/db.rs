use sqlx::{MySqlPool, Row};

use crate::models::TableInfo;

pub async fn create_pool(conn: &str) -> Result<MySqlPool, sqlx::Error> {
    MySqlPool::connect(conn).await
}

pub async fn list_databases(pool: &MySqlPool) -> Result<Vec<String>, sqlx::Error> {
    let rows = sqlx::query("SHOW DATABASES").fetch_all(pool).await?;

    let mut dbs = Vec::new();

    for row in rows {
        let name: String = row.get(0);

        dbs.push(name);
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
