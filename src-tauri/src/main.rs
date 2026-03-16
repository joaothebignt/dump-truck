mod db;
mod models;

use sqlx::MySqlPool;
use std::io::ErrorKind;
use std::process::Command;

use db::*;

fn parse_mysql_conn(conn: &str) -> Result<(String, u16, String, String), String> {
    // expected format: mysql://user:pass@host:port/
    let without_scheme = conn
        .strip_prefix("mysql://")
        .ok_or_else(|| "invalid mysql connection string".to_string())?;

    let (user_pass, host_part) = without_scheme
        .split_once('@')
        .ok_or_else(|| "invalid mysql connection string".to_string())?;

    let (user, pass) = user_pass
        .split_once(':')
        .ok_or_else(|| "invalid mysql connection string".to_string())?;

    let host_part = host_part.trim_end_matches('/');

    let (host, port_str) = host_part
        .split_once(':')
        .ok_or_else(|| "invalid mysql connection string".to_string())?;

    let port: u16 = port_str
        .parse()
        .map_err(|_| "invalid port in connection string".to_string())?;

    Ok((host.to_string(), port, user.to_string(), pass.to_string()))
}

#[tauri::command]
async fn connect_db(host: String, port: u16, user: String, pass: String) -> Result<String, String> {
    let conn = format!("mysql://{}:{}@{}:{}/", user, pass, host, port);

    MySqlPool::connect(&conn).await.map_err(|e| e.to_string())?;

    Ok(conn)
}

#[tauri::command]
async fn databases(conn: String) -> Result<Vec<String>, String> {
    let pool = create_pool(&conn).await.map_err(|e| e.to_string())?;

    let dbs = list_databases(&pool).await.map_err(|e| e.to_string())?;

    Ok(dbs)
}

#[tauri::command]
async fn tables_with_size(
    conn: String,
    database: String,
) -> Result<Vec<models::TableInfo>, String> {
    let pool = create_pool(&conn).await.map_err(|e| e.to_string())?;

    let tables = list_tables_with_size(&pool, &database)
        .await
        .map_err(|e| e.to_string())?;

    Ok(tables)
}

#[tauri::command]
async fn dump_tables(
    conn: String,
    database: String,
    tables: Vec<String>,
) -> Result<String, String> {
    if tables.is_empty() {
        return Err("nenhuma tabela selecionada".to_string());
    }

    let (host, port, user, pass) = parse_mysql_conn(&conn)?;

    let db_name = database.clone();
    let selected_tables = tables.clone();

    let output = tauri::async_runtime::spawn_blocking(move || {
        let mut cmd = Command::new("mysqldump");

        cmd.arg("-h")
            .arg(host)
            .arg("-P")
            .arg(port.to_string())
            .arg("-u")
            .arg(user)
            .arg(format!("-p{}", pass))
            .arg(&db_name);

        for t in selected_tables {
            cmd.arg(t);
        }

        cmd.output()
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e: std::io::Error| {
        if e.kind() == ErrorKind::NotFound {
            return "mysqldump não encontrado no PATH. Instale o cliente MySQL/mariadb e garanta que o binário 'mysqldump' esteja disponível."
                .to_string();
        }
        e.to_string()
    })?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    let sql = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;

    Ok(sql)
}

#[tauri::command]
async fn dump_single_table(
    conn: String,
    database: String,
    table: String,
) -> Result<String, String> {
    let (host, port, user, pass) = parse_mysql_conn(&conn)?;

    let db_name = database.clone();
    let table_name = table.clone();

    let output = tauri::async_runtime::spawn_blocking(move || {
        let mut cmd = Command::new("mysqldump");

        cmd.arg("-h")
            .arg(host)
            .arg("-P")
            .arg(port.to_string())
            .arg("-u")
            .arg(user)
            .arg(format!("-p{}", pass))
            .arg(&db_name)
            .arg(&table_name);

        cmd.output()
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e: std::io::Error| {
        if e.kind() == ErrorKind::NotFound {
            return "mysqldump não encontrado no PATH. Instale o cliente MySQL/mariadb e garanta que o binário 'mysqldump' esteja disponível."
                .to_string();
        }
        e.to_string()
    })?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    let sql = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;

    Ok(sql)
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            connect_db,
            databases,
            tables_with_size,
            dump_tables,
            dump_single_table
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
