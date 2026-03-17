mod db;
mod dump;
mod models;
mod settings;

use sqlx::MySqlPool;
use std::io::ErrorKind;
use std::process::{Command, Stdio};
use std::sync::Arc;
use tokio::sync::Semaphore;

use tauri::Emitter;

use db::*;

/// Valida se o diretório existe e tem permissão de escrita
fn validate_dump_directory(dir: &std::path::Path) -> Result<(), String> {
    // Verificar se existe
    if !dir.exists() {
        return Err(format!(
            "Diretório de dump '{}' não existe.",
            dir.display()
        ));
    }

    // Verificar se é diretório
    if !dir.is_dir() {
        return Err(format!(
            "'{}' não é um diretório.",
            dir.display()
        ));
    }

    // Testar permissão de escrita
    let test_file = dir.join(".write_validation_test");
    std::fs::write(&test_file, b"test").map_err(|e| {
        match e.kind() {
            std::io::ErrorKind::PermissionDenied => {
                format!(
                    "Sem permissão de escrita no diretório '{}'. Verifique as permissões do sistema de arquivos.",
                    dir.display()
                )
            }
            _ => format!(
                "Erro ao validar permissões de escrita em '{}': {}",
                dir.display(),
                e
            ),
        }
    })?;

    // Remover arquivo de teste
    std::fs::remove_file(&test_file).ok();

    Ok(())
}

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

#[tauri::command]
async fn dump_parallel(
    app: tauri::AppHandle,
    conn: String,
    database: String,
    tables: Vec<models::DumpTableRequest>,
) -> Result<models::DumpResult, String> {
    if tables.is_empty() {
        return Err("nenhuma tabela selecionada".to_string());
    }

    // Criar pool de conexões MySQL com database especificado
    let pool_conn = if conn.ends_with('/') {
        format!("{}{}", conn, database)
    } else {
        format!("{}/{}", conn, database)
    };
    let pool = create_pool(&pool_conn).await.map_err(|e| e.to_string())?;
    let pool = Arc::new(pool);

    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
    let output_dir = dump::create_dump_dir(&timestamp)?;

    // Validar diretório antes de iniciar dumps paralelos
    validate_dump_directory(&output_dir)?;

    let total = tables.len();
    let cfg = settings::load();
    let parallel_limit = cfg.max_parallel_dumps.max(1);
    let semaphore = Arc::new(Semaphore::new(parallel_limit));
    let mut handles = Vec::with_capacity(total);

    for table in tables {
        let sem = Arc::clone(&semaphore);
        let pool = Arc::clone(&pool);
        let database = database.clone();
        let output_dir = output_dir.clone();
        let app_handle = app.clone();

        let handle = tokio::spawn(async move {
            let permit = sem.acquire_owned().await.map_err(|e| e.to_string())?;
            let table_name = table.name.clone();

            let _ = app_handle.emit(
                "dump-progress",
                models::DumpProgress {
                    table: table_name.clone(),
                    status: "dumping".to_string(),
                    completed: 0,
                    total: 0,
                    error: None,
                },
            );

            let result = dump::dump_one_table(&pool, &database, &table, &output_dir).await;
            drop(permit); // Liberar semáforo após concluir

            let (status, error) = match &result {
                Ok(_) => ("done".to_string(), None),
                Err(e) => ("error".to_string(), Some(e.clone())),
            };
            Ok::<_, String>((table_name, status, error, result.map(|_| ())))
        });

        handles.push(handle);
    }

    let mut completed = 0usize;
    let mut permission_errors: Vec<String> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    for handle in handles {
        let (table_name, status, error, result) = handle
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;

        completed += 1;

        let _ = app.emit(
            "dump-progress",
            models::DumpProgress {
                table: table_name.clone(),
                status,
                completed,
                total,
                error: error.clone(),
            },
        );

        if let Err(e) = result {
            let error_msg = format!("{}: {}", table_name, e);

            // Categorizar erros de permissão
            if e.contains("Permissão negada") || e.contains("Permission denied") || e.contains("permissão negada") {
                permission_errors.push(error_msg);
            } else {
                errors.push(error_msg);
            }
        }
    }

    // Retornar erro específico para problemas de permissão
    if !permission_errors.is_empty() {
        let suggestion = "\n\nSugestões:\n• Verifique as permissões do diretório de dumps\n• Tente executar o aplicativo com permissões adequadas\n• Verifique se há espaço em disco disponível";
        return Err(format!(
            "Erro de permissão ao exportar {} tabela(s):\n{}{}",
            permission_errors.len(),
            permission_errors.join("\n"),
            suggestion
        ));
    }

    if !errors.is_empty() {
        return Err(format!(
            "Falha ao exportar {} tabela(s):\n{}",
            errors.len(),
            errors.join("\n")
        ));
    }

    let (individual_sum, final_size, final_path) = dump::merge_files(&output_dir)?;
    let verified = dump::verify_merge(individual_sum, final_size, total);

    Ok(models::DumpResult {
        output_dir: output_dir.to_string_lossy().to_string(),
        final_file: final_path.to_string_lossy().to_string(),
        table_count: total,
        individual_size_sum: individual_sum,
        final_file_size: final_size,
        verified,
    })
}

#[tauri::command]
fn get_settings() -> settings::AppSettings {
    settings::load()
}

#[tauri::command]
fn save_settings(s: settings::AppSettings) -> Result<(), String> {
    settings::save(&s)
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
            import_dump,
            dump_parallel,
            get_settings,
            save_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
