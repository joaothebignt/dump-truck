use sqlx::mysql::MySqlRow;
use sqlx::{MySqlPool, Row};

use crate::models::{DatabaseInfo, TableColumn, TableInfo, TablePreview, TableStructure};

const PREVIEW_LIMIT: u32 = 100;
const CELL_MAX_CHARS: usize = 1000;

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

fn quote_ident(name: &str) -> Result<String, String> {
    if name.is_empty() || name.contains('\0') {
        return Err("Nome inválido.".to_string());
    }

    Ok(format!("`{}`", name.replace('`', "``")))
}

fn truncate_cell(value: &str) -> String {
    let mut chars = value.chars();
    let truncated: String = chars.by_ref().take(CELL_MAX_CHARS).collect();

    if chars.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

fn cell_text(row: &MySqlRow, idx: usize) -> Option<String> {
    match row.try_get::<Option<String>, _>(idx) {
        Ok(value) => return value.map(|text| truncate_cell(&text)),
        Err(_) => {}
    }

    match row.try_get::<Option<Vec<u8>>, _>(idx) {
        Ok(None) => None,
        Ok(Some(bytes)) => Some(match String::from_utf8(bytes) {
            Ok(text) => truncate_cell(&text),
            Err(err) => format!("[binário {} bytes]", err.into_bytes().len()),
        }),
        Err(_) => Some("[valor não legível]".to_string()),
    }
}

pub async fn preview_table(
    pool: &MySqlPool,
    database: &str,
    table: &str,
) -> Result<TablePreview, String> {
    let columns = column_names(pool, database, table).await?;
    if columns.is_empty() {
        return Err("Tabela não encontrada.".to_string());
    }

    // CAST força o protocolo binário a entregar texto. O ORDER BY usa a
    // coluna original (alias `t`), então `id` continua ordenado como número.
    let select_list = columns
        .iter()
        .map(|name| {
            let ident = quote_ident(name)?;
            Ok(format!("CAST(t.{ident} AS CHAR) AS {ident}"))
        })
        .collect::<Result<Vec<_>, String>>()?
        .join(", ");

    let sql = format!(
        "SELECT {select_list} FROM {}.{} AS t ORDER BY t.`id` DESC LIMIT {PREVIEW_LIMIT}",
        quote_ident(database)?,
        quote_ident(table)?
    );

    let rows = sqlx::query(&sql).fetch_all(pool).await.map_err(|e| {
        let msg = e.to_string();
        let lower = msg.to_lowercase();
        if lower.contains("unknown column") && lower.contains("'id'") {
            "Esta tabela não tem a coluna id.".to_string()
        } else {
            msg
        }
    })?;

    let preview_rows = rows
        .iter()
        .map(|row| (0..columns.len()).map(|idx| cell_text(row, idx)).collect())
        .collect();

    Ok(TablePreview {
        columns,
        rows: preview_rows,
        limit: PREVIEW_LIMIT,
    })
}

async fn column_names(
    pool: &MySqlPool,
    database: &str,
    table: &str,
) -> Result<Vec<String>, String> {
    let rows = sqlx::query(
        "
        SELECT COLUMN_NAME
        FROM information_schema.COLUMNS
        WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ?
        ORDER BY ORDINAL_POSITION
        ",
    )
    .bind(database)
    .bind(table)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows
        .into_iter()
        .map(|row| row.get::<String, _>(0))
        .collect())
}

pub async fn table_structure(
    pool: &MySqlPool,
    database: &str,
    table: &str,
) -> Result<TableStructure, String> {
    let rows = sqlx::query(
        "
        SELECT
          COLUMN_NAME,
          COLUMN_TYPE,
          IS_NULLABLE,
          COLUMN_KEY,
          COLUMN_DEFAULT,
          EXTRA
        FROM information_schema.COLUMNS
        WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ?
        ORDER BY ORDINAL_POSITION
        ",
    )
    .bind(database)
    .bind(table)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    if rows.is_empty() {
        return Err("Tabela não encontrada.".to_string());
    }

    let columns = rows
        .into_iter()
        .map(|row| TableColumn {
            name: row.try_get(0).unwrap_or_default(),
            column_type: row.try_get(1).unwrap_or_default(),
            nullable: row
                .try_get::<String, _>(2)
                .map(|value| value.eq_ignore_ascii_case("YES"))
                .unwrap_or(false),
            key: row.try_get(3).unwrap_or_default(),
            default: row.try_get(4).unwrap_or(None),
            extra: row.try_get(5).unwrap_or_default(),
        })
        .collect();

    let qualified = format!("{}.{}", quote_ident(database)?, quote_ident(table)?);
    let create_sql = match sqlx::query(&format!("SHOW CREATE TABLE {qualified}"))
        .fetch_one(pool)
        .await
    {
        Ok(row) => row.try_get::<String, _>(1).unwrap_or_default(),
        Err(_) => {
            let row = sqlx::query(&format!("SHOW CREATE VIEW {qualified}"))
                .fetch_one(pool)
                .await
                .map_err(|e| e.to_string())?;
            row.try_get::<String, _>(1).unwrap_or_default()
        }
    };

    Ok(TableStructure {
        columns,
        create_sql,
    })
}
