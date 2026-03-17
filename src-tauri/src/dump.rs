use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use crate::models::DumpTableRequest;
use bigdecimal::BigDecimal;
use sqlx::{MySqlPool, Row};

/// Conta o total de linhas em uma tabela
async fn count_table_rows(pool: &MySqlPool, table_name: &str) -> Result<u64, String> {
    let query = format!("SELECT COUNT(*) FROM `{}`", table_name);
    let row = sqlx::query(&query)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Erro ao contar linhas da tabela '{}': {}", table_name, e))?;

    row.try_get::<i64, _>(0)
        .map(|v| v as u64)
        .or_else(|_| row.try_get::<u64, _>(0))
        .map_err(|e| format!("Erro ao ler COUNT: {}", e))
}

/// Verifica se um objeto é uma VIEW
async fn check_if_view(pool: &MySqlPool, database: &str, table_name: &str) -> Result<bool, String> {
    let query = "SELECT TABLE_TYPE FROM INFORMATION_SCHEMA.TABLES \
                 WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ?";

    let row = sqlx::query(query)
        .bind(database)
        .bind(table_name)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Erro ao verificar tipo de '{}': {}", table_name, e))?;

    if let Some(row) = row {
        let table_type: String = row.try_get(0)
            .map_err(|e| format!("Erro ao ler TABLE_TYPE: {}", e))?;
        Ok(table_type == "VIEW")
    } else {
        Ok(false)
    }
}

/// Formata um valor SQL baseado no tipo da coluna
fn format_sql_value(row: &sqlx::mysql::MySqlRow, col_idx: usize, data_type: &str) -> Result<String, String> {
    // Normalizar tipo para lowercase
    let dt = data_type.to_lowercase();

    // Tipo BIT - representado como valor binário
    if dt == "bit" {
        if let Ok(Some(bytes)) = row.try_get::<Option<Vec<u8>>, _>(col_idx) {
            if bytes.is_empty() {
                return Ok("NULL".to_string());
            }
            return Ok(format!("0x{}", hex::encode(bytes)));
        } else if let Ok(None) = row.try_get::<Option<Vec<u8>>, _>(col_idx) {
            return Ok("NULL".to_string());
        }
        // Fallback: tentar como inteiro
        if let Ok(Some(val)) = row.try_get::<Option<i64>, _>(col_idx) {
            return Ok(format!("0x{:x}", val));
        } else if let Ok(None) = row.try_get::<Option<i64>, _>(col_idx) {
            return Ok("NULL".to_string());
        }
    }

    // Tipo BOOL/BOOLEAN - sinônimo de tinyint(1)
    if dt == "bool" || dt == "boolean" {
        if let Ok(Some(val)) = row.try_get::<Option<bool>, _>(col_idx) {
            return Ok(if val { "TRUE" } else { "FALSE" }.to_string());
        } else if let Ok(None) = row.try_get::<Option<bool>, _>(col_idx) {
            return Ok("NULL".to_string());
        }
        // Fallback: tratar como inteiro
        if let Ok(Some(val)) = row.try_get::<Option<i8>, _>(col_idx) {
            return Ok(if val != 0 { "TRUE" } else { "FALSE" }.to_string());
        } else if let Ok(None) = row.try_get::<Option<i8>, _>(col_idx) {
            return Ok("NULL".to_string());
        }
    }

    // Tipos espaciais - armazenados como bytes binários WKB
    if dt.contains("geometry") || dt.contains("point") || dt.contains("line")
        || dt.contains("polygon") || dt.contains("multi") || dt == "geom"
    {
        if let Ok(Some(bytes)) = row.try_get::<Option<Vec<u8>>, _>(col_idx) {
            return Ok(format!("0x{}", hex::encode(bytes)));
        } else if let Ok(None) = row.try_get::<Option<Vec<u8>>, _>(col_idx) {
            return Ok("NULL".to_string());
        }
    }

    // Tipo UUID
    if dt == "uuid" {
        if let Ok(Some(val)) = row.try_get::<Option<String>, _>(col_idx) {
            return Ok(format!("'{}'", val.replace('\'', "\\'")));
        } else if let Ok(None) = row.try_get::<Option<String>, _>(col_idx) {
            return Ok("NULL".to_string());
        }
    }

    // Tipos numéricos inteiros
    if dt.contains("int") || dt.contains("serial") {
        // Tentar ler como i64
        if let Ok(Some(val)) = row.try_get::<Option<i64>, _>(col_idx) {
            return Ok(val.to_string());
        } else if let Ok(None) = row.try_get::<Option<i64>, _>(col_idx) {
            return Ok("NULL".to_string());
        }
        // Fallback para u64 (para unsigned)
        if let Ok(Some(val)) = row.try_get::<Option<u64>, _>(col_idx) {
            return Ok(val.to_string());
        } else if let Ok(None) = row.try_get::<Option<u64>, _>(col_idx) {
            return Ok("NULL".to_string());
        }
    }

    // Tipos decimais - MySQL envia como BigDecimal
    if dt.contains("decimal") || dt.contains("numeric") {
        // Tentar como BigDecimal primeiro (tipo nativo do MySQL para DECIMAL)
        if let Ok(Some(val)) = row.try_get::<Option<BigDecimal>, _>(col_idx) {
            return Ok(val.to_string());
        } else if let Ok(None) = row.try_get::<Option<BigDecimal>, _>(col_idx) {
            return Ok("NULL".to_string());
        }
        // Fallback: tentar como String
        if let Ok(Some(val)) = row.try_get::<Option<String>, _>(col_idx) {
            return Ok(val);
        } else if let Ok(None) = row.try_get::<Option<String>, _>(col_idx) {
            return Ok("NULL".to_string());
        }
    }

    // Tipos float/double
    if dt.contains("float") || dt.contains("double") {
        if let Ok(Some(val)) = row.try_get::<Option<f64>, _>(col_idx) {
            return Ok(val.to_string());
        } else if let Ok(None) = row.try_get::<Option<f64>, _>(col_idx) {
            return Ok("NULL".to_string());
        }
    }

    // Tipos de data/hora - usar tipos nativos do MySQL
    if dt == "datetime" || dt == "timestamp" {
        // Tentar como chrono::NaiveDateTime primeiro
        if let Ok(Some(val)) = row.try_get::<Option<chrono::NaiveDateTime>, _>(col_idx) {
            return Ok(format!("'{}'", val.format("%Y-%m-%d %H:%M:%S")));
        } else if let Ok(None) = row.try_get::<Option<chrono::NaiveDateTime>, _>(col_idx) {
            return Ok("NULL".to_string());
        }
        // Fallback: tentar como string
        if let Ok(Some(val)) = row.try_get::<Option<String>, _>(col_idx) {
            return Ok(format!("'{}'", val.replace('\'', "\\'")));
        } else if let Ok(None) = row.try_get::<Option<String>, _>(col_idx) {
            return Ok("NULL".to_string());
        }
    }

    if dt == "date" {
        if let Ok(Some(val)) = row.try_get::<Option<chrono::NaiveDate>, _>(col_idx) {
            return Ok(format!("'{}'", val.format("%Y-%m-%d")));
        } else if let Ok(None) = row.try_get::<Option<chrono::NaiveDate>, _>(col_idx) {
            return Ok("NULL".to_string());
        }
        // Fallback: tentar como string
        if let Ok(Some(val)) = row.try_get::<Option<String>, _>(col_idx) {
            return Ok(format!("'{}'", val.replace('\'', "\\'")));
        } else if let Ok(None) = row.try_get::<Option<String>, _>(col_idx) {
            return Ok("NULL".to_string());
        }
    }

    if dt == "time" {
        // MySQL TIME pode ser lido como chrono::NaiveTime com feature chrono
        if let Ok(Some(val)) = row.try_get::<Option<chrono::NaiveTime>, _>(col_idx) {
            return Ok(format!("'{}'", val.format("%H:%M:%S")));
        } else if let Ok(None) = row.try_get::<Option<chrono::NaiveTime>, _>(col_idx) {
            return Ok("NULL".to_string());
        }
        // Fallback: tentar como string
        if let Ok(Some(val)) = row.try_get::<Option<String>, _>(col_idx) {
            return Ok(format!("'{}'", val.replace('\'', "\\'")));
        } else if let Ok(None) = row.try_get::<Option<String>, _>(col_idx) {
            return Ok("NULL".to_string());
        }
    }

    // Year é tratado como inteiro
    if dt == "year" {
        if let Ok(Some(val)) = row.try_get::<Option<i16>, _>(col_idx) {
            return Ok(val.to_string());
        } else if let Ok(None) = row.try_get::<Option<i16>, _>(col_idx) {
            return Ok("NULL".to_string());
        }
    }

    // Tipos binários (blob, binary, varbinary)
    if dt.contains("blob") || dt.contains("binary") {
        if let Ok(Some(bytes)) = row.try_get::<Option<Vec<u8>>, _>(col_idx) {
            return Ok(format!("0x{}", hex::encode(bytes)));
        } else if let Ok(None) = row.try_get::<Option<Vec<u8>>, _>(col_idx) {
            return Ok("NULL".to_string());
        }
    }

    // Tipos de texto e outros (varchar, char, text, json, enum, set, etc)
    // Tentar como String primeiro
    if let Ok(Some(val)) = row.try_get::<Option<String>, _>(col_idx) {
        let escaped = val
            .replace('\\', "\\\\")
            .replace('\'', "\\'")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\0', "\\0");
        return Ok(format!("'{}'", escaped));
    } else if let Ok(None) = row.try_get::<Option<String>, _>(col_idx) {
        return Ok("NULL".to_string());
    }

    // Fallback universal: tentar String, depois bytes, depois NULL como último recurso
    // NUNCA retornar erro - sempre produzir um valor válido

    // 1. Tentar como String (cobre texto, JSON, ENUM, SET, etc.)
    if let Ok(Some(val)) = row.try_get::<Option<String>, _>(col_idx) {
        let escaped = val
            .replace('\\', "\\\\")
            .replace('\'', "\\'")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\0', "\\0");
        return Ok(format!("'{}'", escaped));
    }
    if let Ok(None) = row.try_get::<Option<String>, _>(col_idx) {
        return Ok("NULL".to_string());
    }

    // 2. Tentar como bytes binários
    if let Ok(Some(bytes)) = row.try_get::<Option<Vec<u8>>, _>(col_idx) {
        match String::from_utf8(bytes.clone()) {
            Ok(s) => {
                let escaped = s
                    .replace('\\', "\\\\")
                    .replace('\'', "\\'")
                    .replace('\n', "\\n")
                    .replace('\r', "\\r")
                    .replace('\0', "\\0");
                Ok(format!("'{}'", escaped))
            }
            Err(_) => Ok(format!("0x{}", hex::encode(bytes)))
        }
    } else if let Ok(None) = row.try_get::<Option<Vec<u8>>, _>(col_idx) {
        Ok("NULL".to_string())
    } else {
        // 3. Último recurso: NULL literal
        // Se chegou aqui, o valor é NULL ou tipo totalmente desconhecido
        eprintln!("AVISO: Tipo de coluna '{}' não reconhecido, assumindo NULL", data_type);
        Ok("NULL".to_string())
    }
}

/// Tenta criar e validar um diretório com teste de escrita
fn try_create_and_validate_dir(path: &PathBuf) -> Result<PathBuf, String> {
    // 1. Criar diretório
    std::fs::create_dir_all(path).map_err(|e| {
        format!(
            "Falha ao criar diretório '{}': {}",
            path.display(),
            match e.kind() {
                std::io::ErrorKind::PermissionDenied => "permissão negada".to_string(),
                _ => e.to_string(),
            }
        )
    })?;

    // 2. Definir permissões 0755 em Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(path, perms).map_err(|e| {
            format!(
                "Falha ao definir permissões para '{}': {}",
                path.display(),
                e
            )
        })?;
    }

    // 3. Testar escrita com arquivo temporário
    let test_file = path.join(".write_test");
    std::fs::write(&test_file, b"test").map_err(|e| {
        format!(
            "Diretório '{}' não tem permissão de escrita: {}",
            path.display(),
            match e.kind() {
                std::io::ErrorKind::PermissionDenied => "permissão negada".to_string(),
                _ => e.to_string(),
            }
        )
    })?;

    // 4. Remover arquivo de teste
    std::fs::remove_file(&test_file).ok(); // Ignora erro na remoção

    Ok(path.clone())
}

pub fn create_dump_dir(timestamp: &str, base_dir: &Path) -> Result<PathBuf, String> {
    // Usar o diretório base fornecido (do Tauri app_config_dir)
    let dump_dir = base_dir.join("dumps").join(timestamp);

    match try_create_and_validate_dir(&dump_dir) {
        Ok(dir) => {
            eprintln!("✓ Usando diretório para dumps: {}", dir.display());
            Ok(dir)
        }
        Err(e) => {
            // Fallback para tmp se o diretório de config falhar
            let fallback = PathBuf::from("/tmp")
                .join("dumper-dumps")
                .join(timestamp);

            match try_create_and_validate_dir(&fallback) {
                Ok(dir) => {
                    eprintln!("⚠ Diretório de config não disponível, usando fallback: {}", dir.display());
                    Ok(dir)
                }
                Err(_) => Err(format!(
                    "Falha ao criar diretório de dump:\n  • Config: {}\n  • Fallback: {}\nVerifique as permissões.",
                    e,
                    fallback.display()
                ))
            }
        }
    }
}

/// Cria diretório de dump usando configurações do aplicativo
/// Se custom_dump_dir estiver configurado, tenta usar ele primeiro, depois fallback
pub fn create_dump_dir_with_settings(
    timestamp: &str,
    settings: &crate::settings::AppSettings,
    base_dir: &Path,
) -> Result<PathBuf, String> {
    // Se tem diretório customizado configurado, tentar ele primeiro
    if let Some(custom_dir) = &settings.custom_dump_dir {
        let custom_path = PathBuf::from(custom_dir).join(timestamp);
        match try_create_and_validate_dir(&custom_path) {
            Ok(dir) => {
                eprintln!("✓ Usando diretório customizado para dumps: {}", dir.display());
                return Ok(dir);
            }
            Err(e) => {
                eprintln!(
                    "⚠ Falha ao usar diretório customizado '{}': {}",
                    custom_path.display(),
                    e
                );
                eprintln!("  Tentando locais padrão...");
            }
        }
    }

    // Usar sistema de fallback padrão
    create_dump_dir(timestamp, base_dir)
}

/// Converte um slice de rows em statements INSERT INTO SQL
fn rows_to_insert_sql(
    table_name: &str,
    column_list: &str,
    columns: &[(String, String)],
    rows: &[sqlx::mysql::MySqlRow],
    batch_size: usize,
) -> Result<String, String> {
    let mut sql = String::new();

    for chunk in rows.chunks(batch_size) {
        sql.push_str(&format!(
            "INSERT INTO `{}` ({}) VALUES\n",
            table_name, column_list
        ));

        for (i, row) in chunk.iter().enumerate() {
            sql.push('(');

            for (j, (_col_name, data_type)) in columns.iter().enumerate() {
                if j > 0 {
                    sql.push_str(", ");
                }
                sql.push_str(&format_sql_value(row, j, data_type)?);
            }

            sql.push(')');
            if i < chunk.len() - 1 {
                sql.push_str(",\n");
            }
        }

        sql.push_str(";\n\n");
    }

    Ok(sql)
}

/// Obtém info da Primary Key (coluna única numérica)
async fn get_primary_key_info(
    pool: &MySqlPool,
    database: &str,
    table_name: &str,
) -> Result<Option<(String, String)>, String> {
    // KEY_COLUMN_USAGE não tem DATA_TYPE, precisamos fazer join com COLUMNS
    let query = r#"
        SELECT k.COLUMN_NAME, c.DATA_TYPE
        FROM INFORMATION_SCHEMA.KEY_COLUMN_USAGE k
        JOIN INFORMATION_SCHEMA.COLUMNS c
            ON k.TABLE_SCHEMA = c.TABLE_SCHEMA
            AND k.TABLE_NAME = c.TABLE_NAME
            AND k.COLUMN_NAME = c.COLUMN_NAME
        WHERE k.TABLE_SCHEMA = ?
            AND k.TABLE_NAME = ?
            AND k.CONSTRAINT_NAME = 'PRIMARY'
        ORDER BY k.ORDINAL_POSITION
    "#;

    let rows = sqlx::query(query)
        .bind(database)
        .bind(table_name)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Erro ao obter PK da tabela '{}': {}", table_name, e))?;

    // Só usar PK se for de coluna única e tipo numérico
    if rows.len() == 1 {
        let col_name: String = rows[0].try_get(0)
            .map_err(|e| format!("Erro ao ler COLUMN_NAME: {}", e))?;
        let col_type: String = rows[0].try_get(1)
            .map_err(|e| format!("Erro ao ler DATA_TYPE: {}", e))?;

        // Verificar se tipo é numérico (int, bigint, tinyint, smallint, mediumint)
        let col_type_lower = col_type.to_lowercase();
        if col_type_lower.contains("int") {
            return Ok(Some((col_name, col_type)));
        }
    }
    Ok(None)
}

/// Obtém min/max do PK para chunking por range
async fn get_pk_range(
    pool: &MySqlPool,
    table_name: &str,
    pk_column: &str,
) -> Result<(i64, i64), String> {
    let query = format!(
        "SELECT MIN(`{}`), MAX(`{}`) FROM `{}`",
        pk_column, pk_column, table_name
    );

    let row = sqlx::query(&query)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Erro ao obter range do PK '{}': {}", pk_column, e))?;

    // Tentar ler como i64 (signed), se falhar tentar como u64 (unsigned)
    let min_val: i64 = match row.try_get::<Option<i64>, _>(0) {
        Ok(Some(v)) => v,
        Ok(None) => return Ok((0, 0)), // Tabela vazia
        Err(_) => {
            // Tentar como u64 para colunas UNSIGNED
            match row.try_get::<Option<u64>, _>(0) {
                Ok(Some(v)) => v as i64,
                Ok(None) => return Ok((0, 0)),
                Err(e) => return Err(format!("Erro ao ler MIN do PK: {}", e)),
            }
        }
    };

    let max_val: i64 = match row.try_get::<Option<i64>, _>(1) {
        Ok(Some(v)) => v,
        Ok(None) => return Ok((0, 0)), // Tabela vazia
        Err(_) => {
            // Tentar como u64 para colunas UNSIGNED
            match row.try_get::<Option<u64>, _>(1) {
                Ok(Some(v)) => v as i64,
                Ok(None) => return Ok((0, 0)),
                Err(e) => return Err(format!("Erro ao ler MAX do PK: {}", e)),
            }
        }
    };

    Ok((min_val, max_val))
}

/// Dump de um chunk para arquivo temporário
async fn dump_table_chunk(
    pool: &MySqlPool,
    _database: &str,
    table_name: &str,
    columns: &[(String, String)],
    column_list: &str,
    chunk_id: usize,
    start: u64,
    size: usize,
    use_pk_range: bool,
    pk_column: Option<&str>,
    output_dir: &Path,
    batch_size: usize,
) -> Result<(PathBuf, u64), String> {
    let chunk_path = output_dir.join(format!("{}.chunk_{}.sql", table_name, chunk_id));
    let file = std::fs::File::create(&chunk_path).map_err(|e| {
        format!("Erro ao criar chunk '{}': {}", chunk_path.display(), e)
    })?;
    let mut writer = BufWriter::new(file);
    let mut total_written: u64 = 0;

    // Query baseada na estratégia
    let data_query = if use_pk_range {
        if let Some(pk_col) = pk_column {
            format!(
                "SELECT * FROM `{}` WHERE `{}` >= {} AND `{}` < {}",
                table_name, pk_col, start, pk_col, start + size as u64
            )
        } else {
            // Fallback para OFFSET
            format!(
                "SELECT * FROM `{}` LIMIT {} OFFSET {}",
                table_name, size, start
            )
        }
    } else {
        format!(
            "SELECT * FROM `{}` LIMIT {} OFFSET {}",
            table_name, size, start
        )
    };

    let data_rows = sqlx::query(&data_query)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Erro ao obter dados do chunk: {}", e))?;

    if !data_rows.is_empty() {
        let insert_sql = rows_to_insert_sql(
            table_name,
            column_list,
            columns,
            &data_rows,
            batch_size,
        )?;
        writer.write_all(insert_sql.as_bytes())
            .map_err(|e| format!("Erro de I/O ao escrever chunk: {}", e))?;
        total_written += insert_sql.len() as u64;
    }

    writer.flush().map_err(|e| format!("Erro de I/O ao finalizar chunk: {}", e))?;
    Ok((chunk_path, total_written))
}

/// Merge dos chunks em ordem
fn merge_chunk_files(
    output_dir: &Path,
    table_name: &str,
    chunk_count: usize,
    drop_sql: &str,
    create_sql: &str,
) -> Result<PathBuf, String> {
    let final_path = output_dir.join(format!("{}.sql", table_name));
    let out_file = std::fs::File::create(&final_path).map_err(|e| {
        format!("Erro ao criar arquivo final '{}': {}", final_path.display(), e)
    })?;
    let mut writer = BufWriter::new(out_file);

    // Header (DROP/CREATE)
    writer.write_all(drop_sql.as_bytes())
        .map_err(|e| format!("Erro de I/O: {}", e))?;
    writer.write_all(create_sql.as_bytes())
        .map_err(|e| format!("Erro de I/O: {}", e))?;

    // Chunks em ordem
    for i in 0..chunk_count {
        let chunk_path = output_dir.join(format!("{}.chunk_{}.sql", table_name, i));
        if chunk_path.exists() {
            let in_file = std::fs::File::open(&chunk_path)
                .map_err(|e| format!("Erro ao abrir chunk '{}': {}", chunk_path.display(), e))?;
            let mut reader = BufReader::new(in_file);
            std::io::copy(&mut reader, &mut writer)
                .map_err(|e| format!("Erro ao copiar chunk '{}': {}", chunk_path.display(), e))?;
            std::fs::remove_file(&chunk_path).ok(); // Cleanup
        }
    }

    // Remover header temporário se existir
    let header_path = output_dir.join(format!("{}.header.sql", table_name));
    if header_path.exists() {
        std::fs::remove_file(&header_path).ok();
    }

    writer.flush().map_err(|e| format!("Erro de I/O ao finalizar: {}", e))?;
    Ok(final_path)
}

/// RAII guard para cleanup de arquivos temporários
struct ChunkFileGuard {
    paths: Vec<PathBuf>,
    cleanup: bool,
}

impl ChunkFileGuard {
    fn new(paths: Vec<PathBuf>) -> Self {
        ChunkFileGuard { paths, cleanup: true }
    }

    fn keep(mut self) {
        self.cleanup = false;
    }
}

impl Drop for ChunkFileGuard {
    fn drop(&mut self) {
        if self.cleanup {
            for path in &self.paths {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

/// Dump paralelo de tabela grande dividida em chunks
pub async fn dump_one_table_parallel<F>(
    pool: &MySqlPool,
    database: &str,
    table: &DumpTableRequest,
    output_dir: &Path,
    chunk_size: usize,
    max_parallel_chunks: usize,
    mut progress_callback: F,
) -> Result<u64, String>
where
    F: FnMut(u64, u64) + Send + Clone + 'static,
{
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;
    use tokio::sync::Semaphore;

    // Contar linhas totais
    let total_rows = count_table_rows(pool, &table.name).await?;

    // Verificar se é VIEW
    let is_view = check_if_view(pool, database, &table.name).await?;
    let mut table_request = table.clone();
    if is_view {
        table_request.content = false;
    }

    // Obter estrutura da tabela
    let drop_sql = if table_request.drop {
        format!("DROP TABLE IF EXISTS `{}`;\n\n", table_request.name)
    } else {
        String::new()
    };

    let create_sql = if table_request.structure {
        let create_stmt = sqlx::query(&format!("SHOW CREATE TABLE `{}`", table_request.name))
            .fetch_one(pool)
            .await
            .map_err(|e| format!("Erro ao obter estrutura da tabela '{}': {}", table_request.name, e))?;

        let create: String = create_stmt
            .try_get(1)
            .map_err(|e| format!("Erro ao ler CREATE TABLE: {}", e))?;
        format!("{};\n\n", create)
    } else {
        String::new()
    };

    // Obter colunas
    let columns_query = format!(
        "SELECT COLUMN_NAME, DATA_TYPE FROM INFORMATION_SCHEMA.COLUMNS \
         WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}' \
         ORDER BY ORDINAL_POSITION",
        database, table_request.name
    );

    let column_rows = sqlx::query(&columns_query)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Erro ao obter colunas da tabela '{}': {}", table_request.name, e))?;

    let columns: Vec<(String, String)> = column_rows
        .iter()
        .map(|row| {
            Ok((
                row.try_get::<String, _>(0)?,
                row.try_get::<String, _>(1)?,
            ))
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()
        .map_err(|e| format!("Erro ao ler info de colunas: {}", e))?;

    let column_list = columns
        .iter()
        .map(|(name, _)| format!("`{}`", name))
        .collect::<Vec<_>>()
        .join(", ");

    // Tentar PK Range strategy
    let pk_info = get_primary_key_info(pool, database, &table_request.name).await?;

    let (chunks, use_pk): (Vec<(u64, usize)>, bool) = if let Some((pk_col, _)) = &pk_info {
        // PK Range strategy
        let (min_pk, max_pk) = get_pk_range(pool, &table_request.name, pk_col).await?;
        let pk_range = (max_pk - min_pk) as u64;

        // Calcular número de chunks
        let num_chunks = ((total_rows as f64) / (chunk_size as f64)).ceil() as usize;
        let num_chunks = num_chunks.max(1);

        let chunks: Vec<(u64, usize)> = (0..num_chunks)
            .map(|i| {
                let start = min_pk as u64 + (i as u64) * (pk_range / num_chunks as u64);
                (start, chunk_size)
            })
            .collect();

        (chunks, true)
    } else {
        // OFFSET strategy (fallback)
        let num_chunks = ((total_rows as f64) / (chunk_size as f64)).ceil() as usize;
        let num_chunks = num_chunks.max(1);

        let chunks: Vec<(u64, usize)> = (0..num_chunks)
            .map(|i| {
                ((i as u64) * (chunk_size as u64), chunk_size)
            })
            .collect();

        (chunks, false)
    };

    let num_chunks = chunks.len();

    // Preparar arquivos de chunk para cleanup
    let chunk_paths: Vec<PathBuf> = (0..num_chunks)
        .map(|i| output_dir.join(format!("{}.chunk_{}.sql", table_request.name, i)))
        .collect();
    let guard = ChunkFileGuard::new(chunk_paths.clone());

    // Spawn workers paralelos
    let progress = Arc::new(AtomicU64::new(0));
    let semaphore = Arc::new(Semaphore::new(max_parallel_chunks));
    let pool_arc = Arc::new(pool.clone());
    let columns_arc = Arc::new(columns.clone());
    let column_list_arc = Arc::new(column_list.clone());
    let output_dir_arc = Arc::new(output_dir.to_path_buf());
    let pk_column = pk_info.as_ref().map(|(col, _)| col.clone());
    let table_name = table_request.name.clone();
    let database_str = database.to_string();

    // Coletar dados de chunks em vec owned
    let chunk_data: Vec<(usize, u64, usize)> = chunks.into_iter()
        .enumerate()
        .map(|(id, (start, size))| (id, start, size))
        .collect();

    let mut handles = vec![];

    for (chunk_id, start, size) in chunk_data {
        let sem = Arc::clone(&semaphore);
        let pool = Arc::clone(&pool_arc);
        let columns = Arc::clone(&columns_arc);
        let column_list = Arc::clone(&column_list_arc);
        let output_dir = Arc::clone(&output_dir_arc);
        let prog = Arc::clone(&progress);
        let pk_col = pk_column.clone();
        let table_name = table_name.clone();
        let database = database_str.clone();
        let use_pk_val = use_pk;

        let handle = tokio::spawn(async move {
            let _permit = sem.acquire().await.map_err(|e| e.to_string())?;

            let result = dump_table_chunk(
                &pool,
                &database,
                &table_name,
                &columns,
                &column_list,
                chunk_id,
                start,
                size,
                use_pk_val,
                pk_col.as_deref(),
                &output_dir,
                100, // batch_size
            ).await?;

            // Atualizar progresso
            prog.fetch_add(result.1, Ordering::SeqCst);

            Ok::<_, String>(result)
        });
        handles.push(handle);
    }

    // Aguardar todos os chunks
    for handle in handles {
        let _ = handle.await.map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
    }

    // Merge dos chunks
    let final_path = merge_chunk_files(
        output_dir,
        &table_request.name,
        num_chunks,
        &drop_sql,
        &create_sql,
    )?;

    // Cleanup bem-sucedido - não deletar os arquivos
    guard.keep();

    // Ler tamanho final
    let final_size = final_path.metadata()
        .map(|m| m.len())
        .unwrap_or(0);

    // Notificar progresso final
    progress_callback(total_rows, total_rows);

    Ok(final_size)
}

/// Gera o dump de uma tabela usando sqlx (sem dependência externa)
/// progress_callback: (rows_processed, total_rows) -> should continue
pub async fn dump_one_table<F>(
    pool: &MySqlPool,
    database: &str,
    table: &DumpTableRequest,
    output_dir: &Path,
    page_size: usize,
    parallel_chunk_threshold: u64,
    chunk_size: usize,
    max_parallel_chunks: usize,
    progress_callback: F,
) -> Result<u64, String>
where
    F: FnMut(u64, u64) + Send + Clone + 'static,
{
    // Verificar se é VIEW (views não devem ter dados exportados)
    let is_view = check_if_view(pool, database, &table.name).await?;
    let mut table_request = table.clone();
    if is_view {
        table_request.content = false; // Views: apenas estrutura
    }

    // Se não exporta estrutura nem dados, criar arquivo vazio
    if !table_request.structure && !table_request.content {
        let empty_path = output_dir.join(format!("{}.sql", table_request.name));
        std::fs::write(&empty_path, b"").map_err(|e| {
            match e.kind() {
                std::io::ErrorKind::PermissionDenied => {
                    format!(
                        "Permissão negada ao escrever arquivo '{}'. Verifique as permissões do diretório.",
                        empty_path.display()
                    )
                }
                std::io::ErrorKind::NotFound => {
                    format!(
                        "Diretório '{}' não existe ou foi removido durante a execução.",
                        output_dir.display()
                    )
                }
                _ => format!("Erro de I/O ao escrever '{}': {}", empty_path.display(), e),
            }
        })?;
        return Ok(0);
    }

    // Contar total de linhas para decidir se usa paralelo
    if table_request.content {
        let total_rows = count_table_rows(pool, &table_request.name).await.unwrap_or(0);

        // Se tabela é grande o suficiente, usar dump paralelo
        if total_rows > parallel_chunk_threshold && max_parallel_chunks > 1 {
            return dump_one_table_parallel(
                pool,
                database,
                &table_request,
                output_dir,
                chunk_size,
                max_parallel_chunks,
                progress_callback,
            ).await;
        }
    }

    // Dump sequencial para tabelas pequenas
    dump_one_table_sequential(
        pool,
        database,
        &table_request,
        output_dir,
        page_size,
        progress_callback,
    ).await
}

/// Dump sequencial de uma tabela (método original)
async fn dump_one_table_sequential<F>(
    pool: &MySqlPool,
    database: &str,
    table: &DumpTableRequest,
    output_dir: &Path,
    page_size: usize,
    mut progress_callback: F,
) -> Result<u64, String>
where
    F: FnMut(u64, u64) + Send,
{
    // Abrir arquivo com BufWriter para escrita incremental
    let file_path = output_dir.join(format!("{}.sql", table.name));
    let file = std::fs::File::create(&file_path).map_err(|e| {
        match e.kind() {
            std::io::ErrorKind::PermissionDenied => {
                format!(
                    "Permissão negada ao escrever arquivo '{}'. Verifique as permissões do diretório.",
                    file_path.display()
                )
            }
            std::io::ErrorKind::NotFound => {
                format!(
                    "Diretório '{}' não existe ou foi removido durante a execução.",
                    output_dir.display()
                )
            }
            _ => format!("Erro de I/O ao escrever '{}': {}", file_path.display(), e),
        }
    })?;
    let mut writer = BufWriter::new(file);
    let mut total_written: u64 = 0;

    // DROP TABLE se solicitado
    if table.drop {
        let drop_sql = format!("DROP TABLE IF EXISTS `{}`;\n\n", table.name);
        writer.write_all(drop_sql.as_bytes()).map_err(|e| format!("Erro de I/O: {}", e))?;
        total_written += drop_sql.len() as u64;
    }

    // CREATE TABLE se solicitado
    if table.structure {
        let create_stmt = sqlx::query(&format!("SHOW CREATE TABLE `{}`", table.name))
            .fetch_one(pool)
            .await
            .map_err(|e| format!("Erro ao obter estrutura da tabela '{}': {}", table.name, e))?;

        let create_sql: String = create_stmt
            .try_get(1)
            .map_err(|e| format!("Erro ao ler CREATE TABLE: {}", e))?;

        let structure_sql = format!("{};\n\n", create_sql);
        writer.write_all(structure_sql.as_bytes()).map_err(|e| format!("Erro de I/O: {}", e))?;
        total_written += structure_sql.len() as u64;
    }

    // Dados se solicitado — com paginação
    if table.content {
        // Contar total de linhas para progresso
        let total_rows = count_table_rows(pool, &table.name).await.unwrap_or(0);
        let mut rows_processed: u64 = 0;

        let columns_query = format!(
            "SELECT COLUMN_NAME, DATA_TYPE FROM INFORMATION_SCHEMA.COLUMNS \
             WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}' \
             ORDER BY ORDINAL_POSITION",
            database, table.name
        );

        let column_rows = sqlx::query(&columns_query)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Erro ao obter colunas da tabela '{}': {}", table.name, e))?;

        if !column_rows.is_empty() {
            let columns: Vec<(String, String)> = column_rows
                .iter()
                .map(|row| {
                    Ok((
                        row.try_get::<String, _>(0)?,
                        row.try_get::<String, _>(1)?,
                    ))
                })
                .collect::<Result<Vec<_>, sqlx::Error>>()
                .map_err(|e| format!("Erro ao ler info de colunas: {}", e))?;

            let column_list = columns
                .iter()
                .map(|(name, _)| format!("`{}`", name))
                .collect::<Vec<_>>()
                .join(", ");

            let batch_size = 100;
            let mut offset: u64 = 0;

            loop {
                let data_query = format!(
                    "SELECT * FROM `{}` LIMIT {} OFFSET {}",
                    table.name, page_size, offset
                );
                let data_rows = sqlx::query(&data_query)
                    .fetch_all(pool)
                    .await
                    .map_err(|e| format!("Erro ao obter dados da tabela '{}': {}", table.name, e))?;

                let rows_in_page = data_rows.len() as u64;

                if !data_rows.is_empty() {
                    let insert_sql = rows_to_insert_sql(
                        &table.name,
                        &column_list,
                        &columns,
                        &data_rows,
                        batch_size,
                    )?;
                    writer.write_all(insert_sql.as_bytes()).map_err(|e| format!("Erro de I/O: {}", e))?;
                    total_written += insert_sql.len() as u64;
                }

                // Atualizar progresso
                rows_processed += rows_in_page;
                progress_callback(rows_processed, total_rows);

                if data_rows.len() < page_size {
                    break;
                }
                offset += page_size as u64;
            }
        }
    }

    writer.flush().map_err(|e| format!("Erro de I/O ao finalizar escrita: {}", e))?;

    Ok(total_written)
}

pub fn merge_files(output_dir: &Path) -> Result<(u64, u64, PathBuf), String> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(output_dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension().and_then(|s| s.to_str()) == Some("sql")
                && p.file_name().and_then(|s| s.to_str()) != Some("final_dump.sql")
        })
        .collect();

    entries.sort();

    let final_path = output_dir.join("final_dump.sql");
    let out_file = std::fs::File::create(&final_path).map_err(|e| {
        match e.kind() {
            std::io::ErrorKind::PermissionDenied => {
                format!(
                    "Permissão negada ao criar arquivo '{}'. Verifique as permissões do diretório.",
                    final_path.display()
                )
            }
            std::io::ErrorKind::NotFound => {
                format!(
                    "Diretório '{}' não existe ou foi removido durante a execução.",
                    output_dir.display()
                )
            }
            _ => format!("Erro de I/O ao criar '{}': {}", final_path.display(), e),
        }
    })?;
    let mut writer = BufWriter::new(out_file);

    let mut individual_sum: u64 = 0;
    let mut first = true;

    for entry in &entries {
        let size = entry.metadata().map_err(|e| e.to_string())?.len();
        individual_sum += size;

        if !first {
            writer.write_all(b"\n\n").map_err(|e| e.to_string())?;
        }
        first = false;

        let in_file = std::fs::File::open(entry).map_err(|e| e.to_string())?;
        let mut reader = BufReader::new(in_file);
        std::io::copy(&mut reader, &mut writer).map_err(|e| e.to_string())?;
    }

    writer.flush().map_err(|e| e.to_string())?;

    let final_size = final_path.metadata().map_err(|e| e.to_string())?.len();

    Ok((individual_sum, final_size, final_path))
}

pub fn verify_merge(individual_sum: u64, final_size: u64, table_count: usize) -> bool {
    if table_count == 0 {
        return final_size == 0;
    }
    let separators = (table_count as u64).saturating_sub(1) * 2;
    final_size == individual_sum + separators
}

/// Gera o dump de uma tabela retornando SQL como String (sem escrever arquivo)
pub async fn dump_one_table_to_string(
    pool: &MySqlPool,
    database: &str,
    table: &DumpTableRequest,
    page_size: usize,
) -> Result<String, String> {
    let mut sql_output = String::new();

    // Verificar se é VIEW (views não devem ter dados exportados)
    let is_view = check_if_view(pool, database, &table.name).await?;
    let mut table_request = table.clone();
    if is_view {
        table_request.content = false; // Views: apenas estrutura
    }

    // Se não exporta estrutura nem dados, retornar vazio
    if !table_request.structure && !table_request.content {
        return Ok(String::new());
    }

    // DROP TABLE se solicitado
    if table_request.drop {
        sql_output.push_str(&format!("DROP TABLE IF EXISTS `{}`;\n\n", table_request.name));
    }

    // CREATE TABLE se solicitado
    if table_request.structure {
        let create_stmt = sqlx::query(&format!("SHOW CREATE TABLE `{}`", table_request.name))
            .fetch_one(pool)
            .await
            .map_err(|e| format!("Erro ao obter estrutura da tabela '{}': {}", table_request.name, e))?;

        let create_sql: String = create_stmt
            .try_get(1)
            .map_err(|e| format!("Erro ao ler CREATE TABLE: {}", e))?;

        sql_output.push_str(&create_sql);
        sql_output.push_str(";\n\n");
    }

    // Dados se solicitado — com paginação
    if table_request.content {
        let columns_query = format!(
            "SELECT COLUMN_NAME, DATA_TYPE FROM INFORMATION_SCHEMA.COLUMNS \
             WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}' \
             ORDER BY ORDINAL_POSITION",
            database, table_request.name
        );

        let column_rows = sqlx::query(&columns_query)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Erro ao obter colunas da tabela '{}': {}", table_request.name, e))?;

        if !column_rows.is_empty() {
            let columns: Vec<(String, String)> = column_rows
                .iter()
                .map(|row| {
                    Ok((
                        row.try_get::<String, _>(0)?,
                        row.try_get::<String, _>(1)?,
                    ))
                })
                .collect::<Result<Vec<_>, sqlx::Error>>()
                .map_err(|e| format!("Erro ao ler info de colunas: {}", e))?;

            let column_list = columns
                .iter()
                .map(|(name, _)| format!("`{}`", name))
                .collect::<Vec<_>>()
                .join(", ");

            let batch_size = 100;
            let mut offset: u64 = 0;

            loop {
                let data_query = format!(
                    "SELECT * FROM `{}` LIMIT {} OFFSET {}",
                    table_request.name, page_size, offset
                );
                let data_rows = sqlx::query(&data_query)
                    .fetch_all(pool)
                    .await
                    .map_err(|e| format!("Erro ao obter dados da tabela '{}': {}", table_request.name, e))?;

                if !data_rows.is_empty() {
                    sql_output.push_str(&rows_to_insert_sql(
                        &table_request.name,
                        &column_list,
                        &columns,
                        &data_rows,
                        batch_size,
                    )?);
                }

                if data_rows.len() < page_size {
                    break;
                }
                offset += page_size as u64;
            }
        }
    }

    Ok(sql_output)
}
