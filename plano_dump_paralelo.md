# Plano: Dump Paralelo de Tabelas via Tokio

## Contexto

Atualmente o dump de tabelas é feito de forma **sequencial** pelo frontend — ele chama `dump_single_table` em loop, uma tabela por vez, acumulando SQL em memória e salvando tudo em um único arquivo via dialog de save. Isso é lento para bancos com muitas tabelas.

O objetivo é mover toda a orquestração para o backend Rust, executando dumps em **paralelo via tokio**, escrevendo cada tabela em seu próprio arquivo `.sql`, e ao final unindo tudo em `final_dump.sql` com verificação de integridade.

---

## Arquivos a Modificar

| Arquivo | Ação |
|---|---|
| `src-tauri/Cargo.toml` | Adicionar `chrono` e `dirs` |
| `src-tauri/src/models.rs` | Adicionar structs `DumpTableRequest`, `DumpProgress`, `DumpResult` |
| `src-tauri/src/dump.rs` | Reescrever com funções de dump paralelo, merge e verificação |
| `src-tauri/src/main.rs` | Adicionar comando `dump_parallel`, registrar no handler |
| `src/components/DumpExport.vue` | Substituir loop sequencial por invoke único + listener de eventos |

---

## Steps

### Step 1: Adicionar dependências no `Cargo.toml`

- [ ] Adicionar `chrono = "0.4"` (formatação do timestamp)
- [ ] Adicionar `dirs = "5"` (resolver `~/.config/` cross-platform)

### Step 2: Novos models em `src-tauri/src/models.rs`

- [ ] `DumpTableRequest` — recebido do frontend com `name`, `structure`, `content`, `drop`
  ```rust
  #[derive(Serialize, Deserialize, Clone)]
  pub struct DumpTableRequest {
      pub name: String,
      pub structure: bool,
      pub content: bool,
      pub drop: bool,
  }
  ```
- [ ] `DumpProgress` — emitido via evento Tauri para atualizar o frontend
  ```rust
  #[derive(Serialize, Clone)]
  pub struct DumpProgress {
      pub table: String,
      pub status: String,       // "done" | "error"
      pub completed: usize,
      pub total: usize,
      pub error: Option<String>,
  }
  ```
- [ ] `DumpResult` — retorno do comando com resultado e verificação
  ```rust
  #[derive(Serialize, Clone)]
  pub struct DumpResult {
      pub output_dir: String,
      pub final_file: String,
      pub table_count: usize,
      pub individual_size_sum: u64,
      pub final_file_size: u64,
      pub verified: bool,
  }
  ```

### Step 3: Reescrever `src-tauri/src/dump.rs`

- [ ] **`create_dump_dir(timestamp: &str) -> Result<PathBuf, String>`**
  - Usa `dirs::config_dir()` para resolver `~/.config/`
  - Cria `munhoz-dumper/dumps/{timestamp}/` com `create_dir_all`

- [ ] **`dump_one_table(host, port, user, pass, database, table: &DumpTableRequest, output_dir: &Path) -> Result<u64, String>`**
  - Executa `mysqldump` com as flags baseadas em `structure`/`content`/`drop` (mesma lógica do `dump_single_table` atual)
  - Escreve output direto em `{output_dir}/{table_name}.sql` via `std::fs::write`
  - Retorna o tamanho em bytes do arquivo escrito
  - Trata erro `NotFound` do mysqldump com mensagem em português

- [ ] **`merge_files(output_dir: &Path) -> Result<(u64, u64, PathBuf), String>`**
  - Lista todos os `.sql` (exceto `final_dump.sql`), ordena alfabeticamente
  - Usa `BufReader`/`BufWriter` + `std::io::copy` para concatenar sem carregar tudo em memória
  - Adiciona `\n\n` entre cada arquivo
  - Retorna `(soma_tamanhos_individuais, tamanho_final, path_final)`

- [ ] **`verify_merge(individual_sum: u64, final_size: u64, table_count: usize) -> bool`**
  - Verifica se `final_size == individual_sum + (table_count * 2)` (2 bytes por `\n\n` separador)

### Step 4: Novo comando `dump_parallel` em `src-tauri/src/main.rs`

- [ ] Adicionar `use dump::*;` (ou imports seletivos) — atualmente o mod dump não é importado
- [ ] Criar o comando:
  ```rust
  #[tauri::command]
  async fn dump_parallel(
      app: tauri::AppHandle,
      conn: String,
      database: String,
      tables: Vec<models::DumpTableRequest>,
  ) -> Result<models::DumpResult, String>
  ```
- [ ] Fluxo interno:
  1. Parsear conexão com `parse_mysql_conn`
  2. Gerar timestamp com `chrono::Local::now().format("%Y%m%d_%H%M%S")`
  3. Criar diretório com `create_dump_dir`
  4. Usar `Arc<Semaphore>` com limit de **8** para não sobrecarregar o MySQL
  5. Spawnar uma task `spawn_blocking` por tabela, cada uma adquirindo permit do semaphore
  6. Coletar resultados com `JoinHandle`, emitindo `app.emit("dump-progress", ...)` conforme cada task termina
  7. Se houve erros, retornar `Err` com lista de tabelas que falharam
  8. Chamar `merge_files` para unir em `final_dump.sql`
  9. Chamar `verify_merge` para validar
  10. Retornar `DumpResult`
- [ ] Registrar `dump_parallel` no `generate_handler![]`

### Step 5: Atualizar frontend `src/components/DumpExport.vue`

- [ ] Importar `listen` de `@tauri-apps/api/event`
- [ ] Remover imports de `save` (dialog) e `writeTextFile` (fs) — backend agora escreve direto
- [ ] Substituir `exportDump`:
  - Montar array de `{ name, structure, content, drop }` das tabelas selecionadas
  - Registrar listener em `"dump-progress"` para atualizar `progress` e `currentTable`
  - Chamar `invoke("dump_parallel", { conn, database, tables })`
  - Mostrar resultado: se `verified`, exibir caminho do arquivo; senão, avisar que verificação falhou
  - No `finally`, chamar `unlisten()` e resetar estado
- [ ] Remover `cancelled`/`cancelExport` (cancelamento de tasks paralelas requer mecanismo diferente — implementar depois se necessário)
- [ ] Atualizar template para mostrar o caminho do resultado ao final

### Step 6: Verificação / Testes

- [ ] Compilar o projeto com `cargo build` no `src-tauri/`
- [ ] Testar com um banco real:
  - Selecionar múltiplas tabelas e exportar
  - Verificar que a pasta `~/.config/munhoz-dumper/dumps/{timestamp}/` foi criada
  - Verificar que cada tabela tem seu arquivo `.sql`
  - Verificar que `final_dump.sql` existe e contém o SQL de todas as tabelas
  - Verificar que `verified: true` no resultado
- [ ] Testar edge cases:
  - Tabela única
  - Tabela sem conteúdo (só structure)
  - `mysqldump` não encontrado no PATH
  - Banco com muitas tabelas (verificar que o semaphore limita a 8 paralelos)

---

## Notas Técnicas

- **Semaphore**: Limitar a 8 processos `mysqldump` simultâneos evita saturar conexões MySQL e I/O de disco
- **Escrita em disco vs memória**: `cmd.output()` ainda bufferiza o stdout inteiro em memória por tabela — para tabelas muito grandes (GB+), futuramente pode-se usar `Stdio::piped()` + `std::io::copy` para streaming direto ao arquivo
- **Ordem do merge**: Arquivos são concatenados em ordem alfabética. Como o `mysqldump` geralmente inclui `SET FOREIGN_KEY_CHECKS=0`, a ordem não afeta a importação
- **Cross-platform**: `dirs::config_dir()` resolve corretamente em Linux, macOS e Windows
