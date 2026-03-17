use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use crate::models::DumpTableRequest;
use sqlx::{MySqlPool, Row};

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

pub fn create_dump_dir(timestamp: &str) -> Result<PathBuf, String> {
    // Lista de diretórios candidatos em ordem de preferência
    let mut candidates = Vec::new();
    let mut errors = Vec::new();

    // 1. ~/.config/munhoz-dumper/dumps/{timestamp}/
    if let Some(config_dir) = dirs::config_dir() {
        candidates.push((
            config_dir
                .join("munhoz-dumper")
                .join("dumps")
                .join(timestamp),
            "diretório de configuração XDG",
        ));
    }

    // 2. ~/.local/share/munhoz-dumper/dumps/{timestamp}/
    if let Some(data_dir) = dirs::data_local_dir() {
        candidates.push((
            data_dir
                .join("munhoz-dumper")
                .join("dumps")
                .join(timestamp),
            "diretório de dados locais XDG",
        ));
    }

    // 3. ~/munhoz-dumper-dumps/{timestamp}/
    if let Some(home_dir) = dirs::home_dir() {
        candidates.push((
            home_dir.join("munhoz-dumper-dumps").join(timestamp),
            "diretório home do usuário",
        ));
    }

    // 4. /tmp/munhoz-dumper-dumps/{timestamp}/
    candidates.push((
        PathBuf::from("/tmp")
            .join("munhoz-dumper-dumps")
            .join(timestamp),
        "diretório temporário do sistema",
    ));

    // Tentar cada candidato
    for (path, description) in candidates {
        match try_create_and_validate_dir(&path) {
            Ok(dir) => {
                eprintln!("✓ Usando {} para dumps: {}", description, dir.display());
                return Ok(dir);
            }
            Err(e) => {
                errors.push(format!("  • {}: {}", description, e));
            }
        }
    }

    // Se todos falharam, retornar erro detalhado
    Err(format!(
        "Falha ao criar diretório de dump em todos os locais tentados:\n{}",
        errors.join("\n")
    ))
}

/// Cria diretório de dump usando configurações do aplicativo
/// Se custom_dump_dir estiver configurado, tenta usar ele primeiro, depois fallback
pub fn create_dump_dir_with_settings(
    timestamp: &str,
    settings: &crate::settings::AppSettings,
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
    create_dump_dir(timestamp)
}

/// Gera o dump de uma tabela usando sqlx (sem dependência externa)
pub async fn dump_one_table(
    pool: &MySqlPool,
    database: &str,
    table: &DumpTableRequest,
    output_dir: &Path,
) -> Result<u64, String> {
    let mut sql_output = String::new();

    // Se não exporta estrutura nem dados, criar arquivo vazio
    if !table.structure && !table.content {
        let empty_path = output_dir.join(format!("{}.sql", table.name));
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

    // DROP TABLE se solicitado
    if table.drop {
        sql_output.push_str(&format!("DROP TABLE IF EXISTS `{}`;\n\n", table.name));
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

        sql_output.push_str(&create_sql);
        sql_output.push_str(";\n\n");
    }

    // Dados se solicitado
    if table.content {
        // Primeiro, obter as colunas da tabela
        let columns_query = format!(
            "SELECT COLUMN_NAME FROM INFORMATION_SCHEMA.COLUMNS \
             WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}' \
             ORDER BY ORDINAL_POSITION",
            database, table.name
        );

        let column_rows = sqlx::query(&columns_query)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Erro ao obter colunas da tabela '{}': {}", table.name, e))?;

        if !column_rows.is_empty() {
            let columns: Vec<String> = column_rows
                .iter()
                .map(|row| row.try_get::<String, _>(0))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("Erro ao ler nomes de colunas: {}", e))?;

            // Buscar todos os dados
            let data_query = format!("SELECT * FROM `{}`", table.name);
            let data_rows = sqlx::query(&data_query)
                .fetch_all(pool)
                .await
                .map_err(|e| format!("Erro ao obter dados da tabela '{}': {}", table.name, e))?;

            if !data_rows.is_empty() {
                // Gerar INSERTs em batch (múltiplos valores por INSERT)
                let batch_size = 100;
                let column_list = columns
                    .iter()
                    .map(|c| format!("`{}`", c))
                    .collect::<Vec<_>>()
                    .join(", ");

                for chunk in data_rows.chunks(batch_size) {
                    sql_output.push_str(&format!("INSERT INTO `{}` ({}) VALUES\n", table.name, column_list));

                    for (i, row) in chunk.iter().enumerate() {
                        sql_output.push_str("(");

                        for (j, _col) in columns.iter().enumerate() {
                            if j > 0 {
                                sql_output.push_str(", ");
                            }

                            // Obter valor como Option<String>
                            match row.try_get::<Option<Vec<u8>>, _>(j) {
                                Ok(Some(bytes)) => {
                                    // Tentar converter para string
                                    match String::from_utf8(bytes.clone()) {
                                        Ok(s) => {
                                            // Escapar string SQL
                                            let escaped = s
                                                .replace('\\', "\\\\")
                                                .replace('\'', "\\'")
                                                .replace('\n', "\\n")
                                                .replace('\r', "\\r")
                                                .replace('\0', "\\0");
                                            sql_output.push_str(&format!("'{}'", escaped));
                                        }
                                        Err(_) => {
                                            // Dados binários - usar hex
                                            sql_output.push_str(&format!("0x{}", hex::encode(bytes)));
                                        }
                                    }
                                }
                                Ok(None) => {
                                    sql_output.push_str("NULL");
                                }
                                Err(e) => {
                                    return Err(format!("Erro ao ler valor da coluna: {}", e));
                                }
                            }
                        }

                        sql_output.push(')');
                        if i < chunk.len() - 1 {
                            sql_output.push_str(",\n");
                        }
                    }

                    sql_output.push_str(";\n\n");
                }
            }
        }
    }

    // Escrever arquivo
    let file_path = output_dir.join(format!("{}.sql", table.name));
    std::fs::write(&file_path, sql_output.as_bytes()).map_err(|e| {
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

    Ok(sql_output.len() as u64)
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
) -> Result<String, String> {
    let mut sql_output = String::new();

    // Se não exporta estrutura nem dados, retornar vazio
    if !table.structure && !table.content {
        return Ok(String::new());
    }

    // DROP TABLE se solicitado
    if table.drop {
        sql_output.push_str(&format!("DROP TABLE IF EXISTS `{}`;\n\n", table.name));
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

        sql_output.push_str(&create_sql);
        sql_output.push_str(";\n\n");
    }

    // Dados se solicitado
    if table.content {
        // Primeiro, obter as colunas da tabela
        let columns_query = format!(
            "SELECT COLUMN_NAME FROM INFORMATION_SCHEMA.COLUMNS \
             WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}' \
             ORDER BY ORDINAL_POSITION",
            database, table.name
        );

        let column_rows = sqlx::query(&columns_query)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Erro ao obter colunas da tabela '{}': {}", table.name, e))?;

        if !column_rows.is_empty() {
            let columns: Vec<String> = column_rows
                .iter()
                .map(|row| row.try_get::<String, _>(0))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("Erro ao ler nomes de colunas: {}", e))?;

            // Buscar todos os dados
            let data_query = format!("SELECT * FROM `{}`", table.name);
            let data_rows = sqlx::query(&data_query)
                .fetch_all(pool)
                .await
                .map_err(|e| format!("Erro ao obter dados da tabela '{}': {}", table.name, e))?;

            if !data_rows.is_empty() {
                // Gerar INSERTs em batch (múltiplos valores por INSERT)
                let batch_size = 100;
                let column_list = columns
                    .iter()
                    .map(|c| format!("`{}`", c))
                    .collect::<Vec<_>>()
                    .join(", ");

                for chunk in data_rows.chunks(batch_size) {
                    sql_output.push_str(&format!("INSERT INTO `{}` ({}) VALUES\n", table.name, column_list));

                    for (i, row) in chunk.iter().enumerate() {
                        sql_output.push_str("(");

                        for (j, _col) in columns.iter().enumerate() {
                            if j > 0 {
                                sql_output.push_str(", ");
                            }

                            // Obter valor como Option<String>
                            match row.try_get::<Option<Vec<u8>>, _>(j) {
                                Ok(Some(bytes)) => {
                                    // Tentar converter para string
                                    match String::from_utf8(bytes.clone()) {
                                        Ok(s) => {
                                            // Escapar string SQL
                                            let escaped = s
                                                .replace('\\', "\\\\")
                                                .replace('\'', "\\'")
                                                .replace('\n', "\\n")
                                                .replace('\r', "\\r")
                                                .replace('\0', "\\0");
                                            sql_output.push_str(&format!("'{}'", escaped));
                                        }
                                        Err(_) => {
                                            // Dados binários - usar hex
                                            sql_output.push_str(&format!("0x{}", hex::encode(bytes)));
                                        }
                                    }
                                }
                                Ok(None) => {
                                    sql_output.push_str("NULL");
                                }
                                Err(e) => {
                                    return Err(format!("Erro ao ler valor da coluna: {}", e));
                                }
                            }
                        }

                        sql_output.push(')');
                        if i < chunk.len() - 1 {
                            sql_output.push_str(",\n");
                        }
                    }

                    sql_output.push_str(";\n\n");
                }
            }
        }
    }

    Ok(sql_output)
}
