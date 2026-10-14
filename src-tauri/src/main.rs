mod db;
mod models;

use serde::Serialize;
use sqlx::mysql::MySqlConnectOptions;
use sqlx::MySqlPool;
use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::OnceLock;
use tauri::{AppHandle, Emitter};

use db::*;
use models::DumpTableSelection;

#[derive(Clone, Serialize)]
struct DumpProgressPayload {
    table: String,
    index: usize,
    total: usize,
}

#[derive(Clone, Serialize)]
struct CopyProgressPayload {
    phase: String,
    table: String,
    index: usize,
    total: usize,
}

/// Codifica user/password para uso seguro em URLs `mysql://`.
fn percent_encode_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                use std::fmt::Write;
                let _ = write!(out, "%{:02X}", b);
            }
        }
    }
    out
}

fn percent_decode_component(s: &str) -> Result<String, String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3]) {
                if let Ok(decoded) = u8::from_str_radix(hex, 16) {
                    out.push(decoded);
                    i += 3;
                    continue;
                }
            }
        }

        out.push(bytes[i]);
        i += 1;
    }

    String::from_utf8(out).map_err(|e| e.to_string())
}

/// Aceita host puro, `host:porta` ou URL colada (`mysql://`, `https://`, etc.).
fn normalize_host_input(raw: &str, fallback_port: u16) -> (String, u16) {
    let mut host = raw.trim().to_string();

    for prefix in ["mysql://", "mariadb://", "https://", "http://"] {
        if let Some(rest) = host.strip_prefix(prefix) {
            host = rest.to_string();
            break;
        }
    }

    // Descarta credenciais se o usuário colou user:pass@host
    if let Some((_, after_at)) = host.rsplit_once('@') {
        host = after_at.to_string();
    }

    if let Some((h, _)) = host.split_once('/') {
        host = h.to_string();
    }
    if let Some((h, _)) = host.split_once('?') {
        host = h.to_string();
    }

    if let Some((h, p)) = host.rsplit_once(':') {
        if let Ok(port) = p.parse::<u16>() {
            return (h.to_string(), port);
        }
    }

    (host, fallback_port)
}

fn build_mysql_url(user: &str, pass: &str, host: &str, port: u16) -> String {
    format!(
        "mysql://{}:{}@{}:{}/?charset=utf8mb4",
        percent_encode_component(user),
        percent_encode_component(pass),
        host,
        port
    )
}

fn mysqldump_help_text() -> &'static str {
    static HELP: OnceLock<String> = OnceLock::new();
    HELP.get_or_init(|| {
        Command::new("mysqldump")
            .arg("--help")
            .output()
            .map(|o| {
                let mut text = String::from_utf8_lossy(&o.stdout).into_owned();
                text.push_str(&String::from_utf8_lossy(&o.stderr));
                text
            })
            .unwrap_or_default()
    })
}

fn mysqldump_supports(flag: &str) -> bool {
    mysqldump_help_text().contains(flag)
}

/// Flags para dumps com usuários sem LOCK TABLES / RELOAD (comum em hosting)
/// e opções que aceleram dumps remotos (menos round-trips / menos memória).
fn apply_mysqldump_safe_flags(cmd: &mut Command) {
    cmd.arg("--single-transaction")
        .arg("--skip-lock-tables")
        .arg("--set-gtid-purged=OFF")
        .arg("--quick")
        .arg("--extended-insert")
        .arg("--hex-blob")
        .arg("--default-character-set=utf8mb4")
        // Menos bytes na rede e menos trabalho no cliente.
        .arg("--skip-comments")
        .arg("--skip-add-locks")
        // Pacotes maiores → menos round-trips em dumps remotos.
        .arg("--net-buffer-length=1M")
        .arg("--max-allowed-packet=512M")
        // Continua mesmo se alguma tabela/view falhar (ex.: sem SHOW VIEW).
        .arg("--force")
        .arg("--verbose");

    // MySQL 8 cliente → servidor antigo: column-statistics dispara queries
    // caras em INFORMATION_SCHEMA e costuma ser o maior freio em hosts remotos.
    if mysqldump_supports("column-statistics") {
        cmd.arg("--column-statistics=0");
    }
}

fn parse_mysqldump_verbose_table(line: &str) -> Option<String> {
    const MARKERS: &[&str] = &[
        "Retrieving table structure for table ",
        "Dumping data for table ",
        "Retrieving information for table ",
    ];

    for marker in MARKERS {
        if let Some(pos) = line.find(marker) {
            let rest = &line[pos + marker.len()..];
            let name = if let Some(inner) = rest.strip_prefix('`') {
                inner.split('`').next().unwrap_or("").trim()
            } else {
                rest.split_whitespace()
                    .next()
                    .unwrap_or("")
                    .trim_end_matches(['.', ',', ';'])
                    .trim()
            };

            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }

    None
}

fn is_ignorable_mysqldump_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return true;
    }

    // Linhas de --verbose
    if trimmed.starts_with("-- Retrieving")
        || trimmed.starts_with("-- Dumping")
        || trimmed.starts_with("-- Connecting")
        || trimmed.starts_with("-- Disconnecting")
        || trimmed.starts_with("-- Sending")
    {
        return true;
    }

    let lower = trimmed.to_lowercase();
    lower.contains("using a password on the command line")
        || lower.contains("deprecated")
        || lower.contains("compression-algorithms")
        || lower.contains("column-statistics")
        || lower.contains("show view command denied")
        || lower.contains("got error: 1142")
        || lower.contains("error: 1142")
        || lower.contains("command denied to user")
}

fn clean_mysqldump_stderr(stderr: &str) -> String {
    stderr
        .lines()
        .filter(|line| !is_ignorable_mysqldump_line(line))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn run_mysqldump_to_file(
    app: &AppHandle,
    host: &str,
    port: u16,
    user: &str,
    pass: &str,
    database: &str,
    tables: &[String],
    structure: bool,
    content: bool,
    drop: bool,
    output_path: &str,
    append: bool,
    progress_total: usize,
    progress_offset: usize,
) -> Result<(), String> {
    if tables.is_empty() || (!structure && !content) {
        return Ok(());
    }

    if DUMP_CANCELLED.load(Ordering::SeqCst) {
        return Err("CANCELLED".to_string());
    }

    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(!append)
        .append(append)
        .open(output_path)
        .map_err(|e| e.to_string())?;

    if append {
        file.write_all(b"\n\n").map_err(|e| e.to_string())?;
    }

    let mut cmd = Command::new("mysqldump");
    cmd.arg("-h")
        .arg(host)
        .arg("-P")
        .arg(port.to_string())
        .arg("-u")
        .arg(user)
        .arg(format!("-p{}", pass));

    apply_mysqldump_safe_flags(&mut cmd);

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

    cmd.arg(database);
    for table in tables {
        cmd.arg(table);
    }

    cmd.stdout(Stdio::from(file))
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e: std::io::Error| {
        if e.kind() == ErrorKind::NotFound {
            return "mysqldump não encontrado no PATH. Instale o cliente MySQL/mariadb e garanta que o binário 'mysqldump' esteja disponível."
                .to_string();
        }
        e.to_string()
    })?;

    let pid = child.id();
    DUMP_PID.store(pid, Ordering::SeqCst);

    if DUMP_CANCELLED.load(Ordering::SeqCst) {
        let _ = child.kill();
        let _ = child.wait();
        clear_dump_pid(pid);
        return Err("CANCELLED".to_string());
    }

    let stderr_pipe = child.stderr.take();
    let app_progress = app.clone();
    let tables_index: Vec<String> = tables.to_vec();
    let total = progress_total.max(1);

    let stderr_thread = std::thread::spawn(move || {
        let mut collected = String::new();
        let Some(pipe) = stderr_pipe else {
            return collected;
        };

        let reader = BufReader::new(pipe);
        let mut last_table = String::new();

        for line in reader.lines().flatten() {
            collected.push_str(&line);
            collected.push('\n');

            if let Some(table) = parse_mysqldump_verbose_table(&line) {
                if table == last_table {
                    continue;
                }
                last_table = table.clone();

                let index = tables_index
                    .iter()
                    .position(|t| t == &table)
                    .map(|i| progress_offset + i + 1)
                    .unwrap_or(progress_offset.saturating_add(1));

                let _ = app_progress.emit(
                    "dump-progress",
                    DumpProgressPayload {
                        table,
                        index: index.min(total),
                        total,
                    },
                );
            }
        }

        collected
    });

    let status = child.wait().map_err(|e| e.to_string())?;
    clear_dump_pid(pid);

    let stderr_text = stderr_thread.join().unwrap_or_default();

    if DUMP_CANCELLED.load(Ordering::SeqCst) {
        return Err("CANCELLED".to_string());
    }

    if !status.success() {
        let err = clean_mysqldump_stderr(&stderr_text);
        // Com --force, erros de tabela/view específicos não devem abortar o dump.
        if err.is_empty() {
            return Ok(());
        }
        return Err(err);
    }

    Ok(())
}

static DUMP_PID: AtomicU32 = AtomicU32::new(0);
static DUMP_CANCELLED: AtomicBool = AtomicBool::new(false);

fn kill_pid(pid: u32) -> Result<(), String> {
    #[cfg(unix)]
    {
        Command::new("kill")
            .args(["-9", &pid.to_string()])
            .output()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(windows)]
    {
        Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .output()
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn clear_dump_pid(pid: u32) {
    let _ = DUMP_PID.compare_exchange(pid, 0, Ordering::SeqCst, Ordering::SeqCst);
}

#[tauri::command]
fn prepare_dump() {
    DUMP_CANCELLED.store(false, Ordering::SeqCst);
    DUMP_PID.store(0, Ordering::SeqCst);
}

#[tauri::command]
fn cancel_dump() -> Result<(), String> {
    DUMP_CANCELLED.store(true, Ordering::SeqCst);

    let pid = DUMP_PID.swap(0, Ordering::SeqCst);
    if pid != 0 {
        kill_pid(pid)?;
    }

    Ok(())
}

fn parse_mysql_conn(conn: &str) -> Result<(String, u16, String, String), String> {
    // formatos aceitos:
    // - mysql://user:pass@host:port/
    // - mysql://user:pass@host:port/dbname
    // - mysql://user:pass@host:port/?charset=...
    let without_scheme = conn
        .strip_prefix("mysql://")
        .or_else(|| conn.strip_prefix("mariadb://"))
        .ok_or_else(|| "invalid mysql connection string".to_string())?;

    // Usa o último '@' para não quebrar senhas que contenham '@' (já codificadas como %40).
    let (user_pass, host_part) = without_scheme
        .rsplit_once('@')
        .ok_or_else(|| "invalid mysql connection string".to_string())?;

    let (user_enc, pass_enc) = user_pass
        .split_once(':')
        .ok_or_else(|| "invalid mysql connection string".to_string())?;

    let user = percent_decode_component(user_enc)?;
    let pass = percent_decode_component(pass_enc)?;

    // remove possíveis partes após o host:port (path ou query), ex:
    // host:3306/dbname ou host:3306/?charset=utf8mb4
    let host_part = host_part.trim_end_matches('/');
    let host_port = host_part
        .split_once('/')
        .map(|(hp, _)| hp)
        .unwrap_or(host_part);
    let host_port = host_port.split_once('?').map(|(hp, _)| hp).unwrap_or(host_port);

    let (host, port_str) = host_port
        .split_once(':')
        .ok_or_else(|| "invalid mysql connection string".to_string())?;

    let port: u16 = port_str
        .parse()
        .map_err(|_| "invalid port in connection string".to_string())?;

    Ok((host.to_string(), port, user, pass))
}

#[tauri::command]
async fn connect_db(host: String, port: u16, user: String, pass: String) -> Result<String, String> {
    // Se colarem uma URL completa no campo host, extrai os componentes.
    let (host, port, user, pass) = if host.trim().starts_with("mysql://")
        || host.trim().starts_with("mariadb://")
    {
        parse_mysql_conn(host.trim())?
    } else {
        let (host, port) = normalize_host_input(&host, port);
        (host, port, user, pass)
    };

    if host.is_empty() {
        return Err("Host inválido.".to_string());
    }

    // Evita montar URL na mão para conectar: senhas com %, @, #, etc. quebravam o parse do sqlx
    // ("invalid utf-8 sequence...").
    let options = MySqlConnectOptions::new()
        .host(&host)
        .port(port)
        .username(&user)
        .password(&pass)
        .charset("utf8mb4");

    MySqlPool::connect_with(options)
        .await
        .map_err(|e| e.to_string())?;

    Ok(build_mysql_url(&user, &pass, &host, port))
}

#[tauri::command]
async fn check_connection(host: String, port: u16) -> Result<bool, String> {
    let (host, port) = normalize_host_input(&host, port);
    if host.is_empty() {
        return Ok(false);
    }

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
async fn databases(conn: String) -> Result<Vec<models::DatabaseInfo>, String> {
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
            .arg(format!("-p{}", pass));

        apply_mysqldump_safe_flags(&mut cmd);

        cmd.arg(&db_name);

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
        let err = clean_mysqldump_stderr(&String::from_utf8_lossy(&output.stderr));
        return Err(if err.is_empty() {
            "Falha ao gerar o dump.".to_string()
        } else {
            err
        });
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

    if DUMP_CANCELLED.load(Ordering::SeqCst) {
        return Err("CANCELLED".to_string());
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

        apply_mysqldump_safe_flags(&mut cmd);

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

        cmd.arg(&db_name)
            .arg(&table_name)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(|e: std::io::Error| {
            if e.kind() == ErrorKind::NotFound {
                return "mysqldump não encontrado no PATH. Instale o cliente MySQL/mariadb e garanta que o binário 'mysqldump' esteja disponível."
                    .to_string();
            }
            e.to_string()
        })?;

        let pid = child.id();
        DUMP_PID.store(pid, Ordering::SeqCst);

        if DUMP_CANCELLED.load(Ordering::SeqCst) {
            let _ = child.kill();
            let _ = child.wait();
            clear_dump_pid(pid);
            return Err("CANCELLED".to_string());
        }

        let output = child.wait_with_output().map_err(|e| e.to_string())?;
        clear_dump_pid(pid);

        if DUMP_CANCELLED.load(Ordering::SeqCst) {
            return Err("CANCELLED".to_string());
        }

        Ok(output)
    })
    .await
    .map_err(|e| e.to_string())??;

    if !output.status.success() {
        if DUMP_CANCELLED.load(Ordering::SeqCst) {
            return Err("CANCELLED".to_string());
        }
        let err = clean_mysqldump_stderr(&String::from_utf8_lossy(&output.stderr));
        return Err(if err.is_empty() {
            "Falha ao gerar o dump.".to_string()
        } else {
            err
        });
    }

    let sql = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;

    Ok(sql)
}

/// Dump otimizado: agrupa tabelas por opções e gera poucas conexões mysqldump,
/// escrevendo direto no arquivo (bem mais rápido em hosts remotos).
#[tauri::command]
async fn dump_export(
    app: AppHandle,
    conn: String,
    database: String,
    tables: Vec<DumpTableSelection>,
    output_path: String,
) -> Result<(), String> {
    let selected: Vec<DumpTableSelection> = tables
        .into_iter()
        .filter(|t| t.structure || t.content)
        .collect();

    if selected.is_empty() {
        return Err("nenhuma tabela selecionada".to_string());
    }

    if DUMP_CANCELLED.load(Ordering::SeqCst) {
        return Err("CANCELLED".to_string());
    }

    let (host, port, user, pass) = parse_mysql_conn(&conn)?;

    tauri::async_runtime::spawn_blocking(move || {
        dump_table_groups_to_file(
            &app,
            &host,
            port,
            &user,
            &pass,
            &database,
            &selected,
            &output_path,
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

fn run_mysql_import_file(
    host: &str,
    port: u16,
    user: &str,
    pass: &str,
    database: &str,
    path: &std::path::Path,
) -> Result<(), String> {
    if DUMP_CANCELLED.load(Ordering::SeqCst) {
        return Err("CANCELLED".to_string());
    }

    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;

    let mut cmd = Command::new("mysql");
    cmd.arg("-h")
        .arg(host)
        .arg("-P")
        .arg(port.to_string())
        .arg("-u")
        .arg(user)
        .arg(format!("-p{}", pass))
        .arg(database)
        .stdin(Stdio::from(file))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e: std::io::Error| {
        if e.kind() == ErrorKind::NotFound {
            return "mysql não encontrado no PATH. Instale o cliente MySQL/mariadb."
                .to_string();
        }
        e.to_string()
    })?;

    let pid = child.id();
    DUMP_PID.store(pid, Ordering::SeqCst);

    if DUMP_CANCELLED.load(Ordering::SeqCst) {
        let _ = child.kill();
        let _ = child.wait();
        clear_dump_pid(pid);
        return Err("CANCELLED".to_string());
    }

    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    clear_dump_pid(pid);

    if DUMP_CANCELLED.load(Ordering::SeqCst) {
        return Err("CANCELLED".to_string());
    }

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    Ok(())
}

fn dump_table_groups_to_file(
    app: &AppHandle,
    host: &str,
    port: u16,
    user: &str,
    pass: &str,
    database: &str,
    selected: &[DumpTableSelection],
    output_path: &str,
) -> Result<(), String> {
    let mut groups: BTreeMap<(bool, bool, bool), Vec<String>> = BTreeMap::new();

    for table in selected {
        groups
            .entry((table.structure, table.content, table.drop))
            .or_default()
            .push(table.name.clone());
    }

    for names in groups.values_mut() {
        names.sort_by_key(|name| name.to_lowercase());
    }

    let total = selected.len().max(1);
    let mut first = true;
    let mut offset = 0usize;

    for ((structure, content, drop), names) in groups {
        if DUMP_CANCELLED.load(Ordering::SeqCst) {
            return Err("CANCELLED".to_string());
        }

        if let Some(first_table) = names.first() {
            let _ = app.emit(
                "dump-progress",
                DumpProgressPayload {
                    table: first_table.clone(),
                    index: (offset + 1).min(total),
                    total,
                },
            );
        }

        run_mysqldump_to_file(
            app,
            host,
            port,
            user,
            pass,
            database,
            &names,
            structure,
            content,
            drop,
            output_path,
            !first,
            total,
            offset,
        )?;

        offset += names.len();
        first = false;
    }

    Ok(())
}

#[tauri::command]
async fn import_dump(
    conn: String,
    database: String,
    path: String,
) -> Result<(), String> {
    let (host, port, user, pass) = parse_mysql_conn(&conn)?;
    let path = std::path::PathBuf::from(path);

    if !path.exists() {
        return Err("Arquivo não encontrado.".to_string());
    }

    tauri::async_runtime::spawn_blocking(move || {
        run_mysql_import_file(&host, port, &user, &pass, &database, &path)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Copia um database de uma conexão para outro database (já existente) em outra conexão.
#[tauri::command]
async fn copy_database(
    app: AppHandle,
    source_conn: String,
    source_database: String,
    target_conn: String,
    target_database: String,
    tables: Option<Vec<DumpTableSelection>>,
) -> Result<(), String> {
    if source_conn == target_conn && source_database == target_database {
        return Err("Origem e destino são o mesmo database.".to_string());
    }

    DUMP_CANCELLED.store(false, Ordering::SeqCst);
    DUMP_PID.store(0, Ordering::SeqCst);

    let selected = if let Some(list) = tables {
        list.into_iter()
            .filter(|t| t.structure || t.content)
            .collect::<Vec<_>>()
    } else {
        let pool = create_pool(&source_conn)
            .await
            .map_err(|e| e.to_string())?;
        let infos = list_tables_with_size(&pool, &source_database)
            .await
            .map_err(|e| e.to_string())?;
        infos
            .into_iter()
            .map(|t| DumpTableSelection {
                name: t.name,
                structure: true,
                content: true,
                drop: true,
            })
            .collect()
    };

    if selected.is_empty() {
        return Err("Nenhuma tabela para copiar.".to_string());
    }

    let (source_host, source_port, source_user, source_pass) = parse_mysql_conn(&source_conn)?;
    let (target_host, target_port, target_user, target_pass) = parse_mysql_conn(&target_conn)?;

    let temp_path = std::env::temp_dir().join(format!(
        "dumper-copy-{}-{}.sql",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));
    let temp_path_str = temp_path.to_string_lossy().to_string();
    let total = selected.len();

    let dump_result = {
        let app_dump = app.clone();
        let selected_dump = selected.clone();
        let source_database = source_database.clone();
        let temp_path_str = temp_path_str.clone();

        tauri::async_runtime::spawn_blocking(move || {
            let _ = app_dump.emit(
                "copy-progress",
                CopyProgressPayload {
                    phase: "dump".to_string(),
                    table: selected_dump
                        .first()
                        .map(|t| t.name.clone())
                        .unwrap_or_default(),
                    index: 0,
                    total: total.max(1),
                },
            );

            dump_table_groups_to_file(
                &app_dump,
                &source_host,
                source_port,
                &source_user,
                &source_pass,
                &source_database,
                &selected_dump,
                &temp_path_str,
            )?;

            let _ = app_dump.emit(
                "copy-progress",
                CopyProgressPayload {
                    phase: "dump".to_string(),
                    table: "Dump concluído".to_string(),
                    index: total.max(1),
                    total: total.max(1),
                },
            );

            Ok::<(), String>(())
        })
        .await
        .map_err(|e| e.to_string())?
    };

    if let Err(e) = dump_result {
        let _ = std::fs::remove_file(&temp_path);
        return Err(e);
    }

    if DUMP_CANCELLED.load(Ordering::SeqCst) {
        let _ = std::fs::remove_file(&temp_path);
        return Err("CANCELLED".to_string());
    }

    let import_result = {
        let app_import = app.clone();
        let target_database = target_database.clone();
        let temp_path_import = temp_path.clone();

        tauri::async_runtime::spawn_blocking(move || {
            let _ = app_import.emit(
                "copy-progress",
                CopyProgressPayload {
                    phase: "import".to_string(),
                    table: target_database.clone(),
                    index: total.max(1),
                    total: total.max(1),
                },
            );

            run_mysql_import_file(
                &target_host,
                target_port,
                &target_user,
                &target_pass,
                &target_database,
                &temp_path_import,
            )
        })
        .await
        .map_err(|e| e.to_string())?
    };

    let _ = std::fs::remove_file(&temp_path);

    import_result
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
            dump_export,
            prepare_dump,
            cancel_dump,
            import_dump,
            copy_database
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
