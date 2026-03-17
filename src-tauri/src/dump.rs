use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::models::DumpTableRequest;

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

pub fn dump_one_table(
    host: &str,
    port: u16,
    user: &str,
    pass: &str,
    database: &str,
    table: &DumpTableRequest,
    output_dir: &Path,
) -> Result<u64, String> {
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

    let mut cmd = Command::new("mysqldump");

    cmd.arg("-h")
        .arg(host)
        .arg("-P")
        .arg(port.to_string())
        .arg("-u")
        .arg(user)
        .arg(format!("-p{}", pass));

    if table.structure && !table.content {
        cmd.arg("--no-data");
    } else if table.content && !table.structure {
        cmd.arg("--no-create-info");
    }

    if table.drop {
        cmd.arg("--add-drop-table");
    } else {
        cmd.arg("--skip-add-drop-table");
    }

    cmd.arg(database).arg(&table.name);

    let output = cmd.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            "mysqldump não encontrado no PATH. Instale o cliente MySQL/mariadb e garanta que o binário 'mysqldump' esteja disponível.".to_string()
        } else {
            e.to_string()
        }
    })?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    let file_path = output_dir.join(format!("{}.sql", table.name));
    std::fs::write(&file_path, &output.stdout).map_err(|e| {
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

    Ok(output.stdout.len() as u64)
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
