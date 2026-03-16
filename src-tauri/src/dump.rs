use sqlx::{MySqlPool, Row};

pub async fn dump_table(
    pool: &MySqlPool,
    table: &str
) -> Result<String, sqlx::Error> {

    let mut dump = String::new();

    let row = sqlx::query(
        &format!("SHOW CREATE TABLE {}", table)
    )
    .fetch_one(pool)
    .await?;

    let create:String = row.get(1);

    dump.push_str(&format!(
        "DROP TABLE IF EXISTS {};\n{}\n\n",
        table, create
    ));

    let rows = sqlx::query(&format!("SELECT * FROM {}", table))
        .fetch_all(pool)
        .await?;

    for r in rows {

        dump.push_str(&format!(
            "-- row export placeholder\n"
        ));
    }

    Ok(dump)
}