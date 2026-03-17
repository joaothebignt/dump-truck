mod db;
mod models;

use sqlx::MySqlPool;
use std::io::ErrorKind;
use std::process::{Command, Stdio};

use db::*;

fn parse_mysql_conn(conn: &str) -> Result<(String, u16, String, String), String> {
    // formatos aceitos:
    // - mysql://user:pass@host:port/
    // - mysql://user:pass@host:port/dbname
    // - mysql://user:pass@host:port/?charset=...
    let without_scheme = conn
        .strip_prefix("mysql://")
        .ok_or_else(|| "invalid mysql connection string".to_string())?;

    let (user_pass, host_part) = without_scheme
        .split_once('@')
        .ok_or_else(|| "invalid mysql connection string".to_string())?;

    let (user, pass) = user_pass
        .split_once(':')
        .ok_or_else(|| "invalid mysql connection string".to_string())?;

    // remove possíveis partes após o host:port (path ou query), ex:
    // host:3306/dbname ou host:3306/?charset=utf8mb4
    let host_part = host_part.trim_end_matches('/');
    let host_port = host_part
        .split_once('/')
        .map(|(hp, _)| hp)
        .unwrap_or(host_part);

    let (host, port_str) = host_port
        .split_once(':')
        .ok_or_else(|| "invalid mysql connection string".to_string())?;

    let port: u16 = port_str
        .parse()
        .map_err(|_| "invalid port in connection string".to_string())?;

    Ok((host.to_string(), port, user.to_string(), pass.to_string()))
}

#[tauri::command]
async fn connect_db(host: String, port: u16, user: String, pass: String) -> Result<String, String> {
    // Permitir tanto:
    // - host/domínio simples: "db.dominio.com.br"
    // - URL completa: "mysql://user:pass@host:port/" ou ".../dbname?charset=..."
    // Sempre garantimos charset explícito para evitar problemas de UTF‑8.
    let mut conn = if host.starts_with("mysql://") {
        host
    } else {
        format!("mysql://{}:{}@{}:{}/", user, pass, host, port)
    };

    if !conn.contains("charset=") {
        if conn.contains('?') {
            conn.push_str("&charset=utf8mb4");
        } else {
            conn.push_str("?charset=utf8mb4");
        }
    }

    MySqlPool::connect(&conn).await.map_err(|e| e.to_string())?;

    Ok(conn)
}

#[tauri::command]
async fn check_connection(host: String, port: u16) -> Result<bool, String> {
    let addr = format!("{}:{}", host, port);
    match tokio::time::timeout(
        std::time::Duration::from_secs(3),
        tokio::net::TcpStream::connect(&addr),
    )
    .await
    {
        Ok(Ok(_)) => Ok(true),
        Ok(Err(_)) => Ok(false),
        Err(_) => Ok(false),
    }
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
    structure: bool,
    content: bool,
    drop: bool,
) -> Result<String, String> {
    if !structure && !content {
        return Ok(String::new());
    }

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
            .arg(format!("-p{}", pass));

        if structure && !content {
            cmd.arg("--no-data");
        } else if content && !structure {
            cmd.arg("--no-create-info");
        }

        if drop {
            cmd.arg("--add-drop-table");
        } else {
            cmd.arg("--skip-add-drop-table");
        }

        cmd.arg(&db_name).arg(&table_name);

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
async fn import_dump(
    conn: String,
    database: String,
    path: String,
) -> Result<(), String> {
    let (host, port, user, pass) = parse_mysql_conn(&conn)?;

    let db_name = database.clone();
    let path = std::path::PathBuf::from(path);

    if !path.exists() {
        return Err("Arquivo não encontrado.".to_string());
    }

    let file = std::fs::File::open(&path).map_err(|e| e.to_string())?;

    let output = tauri::async_runtime::spawn_blocking(move || {
        let mut cmd = Command::new("mysql");

        cmd.arg("-h")
            .arg(host)
            .arg("-P")
            .arg(port.to_string())
            .arg("-u")
            .arg(user)
            .arg(format!("-p{}", pass))
            .arg(&db_name)
            .stdin(Stdio::from(file))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        cmd.output()
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e: std::io::Error| {
        if e.kind() == ErrorKind::NotFound {
            return "mysql não encontrado no PATH. Instale o cliente MySQL/mariadb."
                .to_string();
        }
        e.to_string()
    })?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            connect_db,
            check_connection,
            databases,
            tables_with_size,
            dump_tables,
            dump_single_table,
            import_dump
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
