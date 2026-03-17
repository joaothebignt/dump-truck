mod db;
mod dump;
mod models;
mod settings;

const PAGE_SIZE: usize = 1000;

use sqlx::MySqlPool;
use std::sync::Arc;
use tokio::sync::Semaphore;

use tauri::{Emitter, Manager};

use db::*;

fn config_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    app.path()
        .app_config_dir()
        .map_err(|e| format!("Não foi possível resolver o diretório de configuração: {}", e))
}

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
    app: tauri::AppHandle,
    conn: String,
    database: String,
) -> Result<Vec<models::TableInfo>, String> {
    let pool = create_pool(&conn).await.map_err(|e| e.to_string())?;

    let cfg = settings::load(&config_dir(&app)?);
    let exclude = &cfg.table_exclude_patterns;

    let all_tables = list_tables_with_size(&pool, &database)
        .await
        .map_err(|e| e.to_string())?;

    let tables = if exclude.is_empty() {
        all_tables
    } else {
        all_tables
            .into_iter()
            .filter(|t| !db::should_exclude_table(&t.name, exclude))
            .collect()
    };

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

    // Criar pool com database
    let pool_conn = format!("{}/{}", conn.trim_end_matches('/'), database);
    let pool = create_pool(&pool_conn).await.map_err(|e| e.to_string())?;

    let mut sql_output = String::new();

    // Dump de cada tabela sequencialmente
    for table in tables {
        let request = models::DumpTableRequest {
            name: table,
            structure: true,
            content: true,
            drop: false,
            size: 0,
        };

        let table_sql = dump::dump_one_table_to_string(&pool, &database, &request, PAGE_SIZE).await?;

        if !sql_output.is_empty() {
            sql_output.push_str("\n\n");
        }
        sql_output.push_str(&table_sql);
    }

    Ok(sql_output)
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

    // Criar pool com database
    let pool_conn = format!("{}/{}", conn.trim_end_matches('/'), database);
    let pool = create_pool(&pool_conn).await.map_err(|e| e.to_string())?;

    // Preparar request
    let request = models::DumpTableRequest {
        name: table,
        structure,
        content,
        drop,
        size: 0,
    };

    // Gerar SQL usando função nativa
    dump::dump_one_table_to_string(&pool, &database, &request, PAGE_SIZE).await
}

/// Parser simples de SQL que divide em statements respeitando strings
fn parse_sql_statements(sql: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut string_char = ' ';
    let mut escape_next = false;
    let mut chars = sql.chars().peekable();

    while let Some(c) = chars.next() {
        if escape_next {
            current.push(c);
            escape_next = false;
            continue;
        }

        match c {
            '\\' => {
                current.push(c);
                escape_next = true;
            }
            '\'' | '"' => {
                current.push(c);
                if in_string && c == string_char {
                    in_string = false;
                } else if !in_string {
                    in_string = true;
                    string_char = c;
                }
            }
            ';' if !in_string => {
                let trimmed = current.trim();
                if !trimmed.is_empty()
                    && !trimmed.starts_with("--")
                    && !trimmed.starts_with("/*") {
                    statements.push(current.clone());
                }
                current.clear();
            }
            _ => {
                current.push(c);
            }
        }
    }

    // Último statement se não termina com ;
    let trimmed = current.trim();
    if !trimmed.is_empty()
        && !trimmed.starts_with("--")
        && !trimmed.starts_with("/*") {
        statements.push(current);
    }

    statements
}

#[tauri::command]
async fn import_dump(
    conn: String,
    database: String,
    path: String,
) -> Result<(), String> {
    let path = std::path::PathBuf::from(path);

    if !path.exists() {
        return Err("Arquivo não encontrado.".to_string());
    }

    // Ler arquivo SQL
    let sql_content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Erro ao ler arquivo: {}", e))?;

    // Criar pool com database
    let pool_conn = format!("{}/{}", conn.trim_end_matches('/'), database);
    let pool = create_pool(&pool_conn).await.map_err(|e| e.to_string())?;

    // Dividir em statements
    let statements = parse_sql_statements(&sql_content);

    // Executar cada statement
    for (idx, stmt) in statements.iter().enumerate() {
        sqlx::query(stmt)
            .execute(&pool)
            .await
            .map_err(|e| format!("Erro no statement {}: {}", idx + 1, e))?;
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
    let cfg_dir = config_dir(&app)?;
    let output_dir = dump::create_dump_dir(&timestamp, &cfg_dir)?;

    // Validar diretório antes de iniciar dumps paralelos
    validate_dump_directory(&output_dir)?;

    // Ordenar tabelas por tamanho (leves primeiro, pesadas por último)
    let mut tables: Vec<_> = tables.into_iter().collect();
    tables.sort_by_key(|t| t.size);

    let total = tables.len();
    let cfg = settings::load(&config_dir(&app)?);
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
            let app_h = app_handle.clone();

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

            let result = dump::dump_one_table(
                &pool,
                &database,
                &table,
                &output_dir,
                PAGE_SIZE,
                cfg.parallel_chunk_threshold,
                cfg.chunk_size,
                cfg.max_parallel_dumps,
                move |rows_processed, total_rows| {
                    let _ = app_h.emit(
                        "dump-progress",
                        models::DumpProgress {
                            table: table_name.clone(),
                            status: "dumping".to_string(),
                            completed: rows_processed as usize,
                            total: total_rows as usize,
                            error: None,
                        },
                    );
                },
            ).await;
            drop(permit); // Liberar semáforo após concluir

            let table_name_result = table.name.clone();
            let (status, error) = match &result {
                Ok(_) => ("done".to_string(), None),
                Err(e) => ("error".to_string(), Some(e.clone())),
            };
            Ok::<_, String>((table_name_result, status, error, result.map(|_| ())))
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
fn get_settings(app: tauri::AppHandle) -> Result<settings::AppSettings, String> {
    Ok(settings::load(&config_dir(&app)?))
}

#[tauri::command]
fn save_settings(app: tauri::AppHandle, s: settings::AppSettings) -> Result<(), String> {
    settings::save(&config_dir(&app)?, &s)
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
