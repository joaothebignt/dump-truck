# AGENTS.md

## Visao Geral

Aplicacao desktop Tauri + Vue 3 para geracao de dumps de bancos de dados MySQL.
Backend em Rust com sqlx, frontend em Vue 3 com Pinia para gerenciamento de estado.
Suporta dump paralelo com controle de concorrencia (Semaphore) e configuracoes persistentes via settings.

## Pre-requisitos

- Node.js 18+
- Rust toolchain (rustc, cargo)
- Cliente MySQL (mysql/mysqldump) disponivel no PATH do sistema

## Comandos de Build e Dev

O `justfile` e a fonte canonica de comandos. Use `just --list` para ver todos os disponiveis.

```bash
# Principais
just dev              # Desenvolvimento com hot reload
just build            # Build de producao (nativo, SO atual)
just build-linux-full # Build Linux completo via container

# Lint e formatacao
just fmt              # Formata Rust e Vue/JS
just fmt-check        # Verifica formatacao sem modificar
just lint             # Verifica formatacao + clippy
just lint-rust        # Apenas clippy

# Limpeza
just clean            # Remove target/, node_modules/, dist/
just clean-all        # Remove tambem lock files
just clean-containers # Remove imagens/containers de build

# CI
just ci               # install + lint + build
just release          # install + fmt + lint + build
```

## Comandos de Teste

O projeto nao possui testes configurados.

## Estilo de Codigo

### JavaScript/Vue

- SFCs com `<script setup>` **exclusivamente**
- Imports com aspas duplas
- **Sem comentarios** no codigo
- snake_case para funcoes e variaveis
- camelCase para props e emits
- PascalCase para componentes

### Extrutura de Diretorios Frontend

```
src/
  components/    # Componentes reutilizaveis
  pages/         # Paginas/rotas
  stores/        # Pinia stores
  router/        # Configuracao do Vue Router
  assets/        # Assets estaticos
  App.vue        # Componente raiz
  main.js        # Entry point
```

### Rust

- snake_case para funcoes, variaveis e modulos
- PascalCase para structs e enums
- `Result<T, String>` para comandos Tauri
- Mensagens de erro em **portugues**
- **Sem comentarios** no codigo
- Each Tauri command decorado com `#[tauri::command]`

### Extrutura de Diretorios Rust

```
src-tauri/src/
  main.rs       # Entry point, comandos Tauri e orquestracao do dump paralelo
  lib.rs        # Configuracao da lib (mobile)
  db.rs         # Operacoes de banco de dados (pool, listagem)
  models.rs     # Structs de dados (TableInfo, DumpTableRequest, DumpProgress, DumpResult)
  dump.rs       # Operacoes de dump paralelo (create_dump_dir, dump_one_table, merge_files, verify_merge)
  settings.rs   # Configuracoes persistentes (AppSettings, max_parallel_dumps)
```

## Arquitetura

### Comunicacao Frontend-Backend

```javascript
// Frontend (Vue)
import { invoke } from "@tauri-apps/api/core"

const resultado = await invoke("nome_comando", {
  param1: valor1,
  param2: valor2
})
```

```rust
// Backend (Rust)
#[tauri::command]
async fn nome_comando(param1: String, param2: u16) -> Result<Tipo, String> {
    // logica aqui
    Ok(resultado)
}
```

### Dump Paralelo

O comando `dump_parallel` orquestra a exportacao de multiplas tabelas em paralelo:

1. Cria diretorio temporario em `~/.config/munhoz-dumper/dumps/<timestamp>/`
2. Carrega `max_parallel_dumps` do settings (default: 5) e cria um `tokio::sync::Semaphore`
3. Spawna uma task por tabela — cada uma adquire um permit do semaphore antes de executar `mysqldump`
4. Emite eventos `dump-progress` via `app.emit()` conforme cada tabela completa (ou falha)
5. Apos todas as tabelas: `merge_files` concatena os `.sql` individuais em `final_dump.sql`
6. `verify_merge` confere que o tamanho do arquivo final bate com a soma dos individuais + separadores

### Settings

Configuracoes persistentes salvas em `~/.config/munhoz-dumper/settings.json`.

- `AppSettings { max_parallel_dumps: usize }` (default: 5)
- Comandos Tauri: `get_settings` e `save_settings`
- Fallback para defaults quando o arquivo nao existe ou e invalido

### Stores Pinia

```javascript
import { defineStore } from "pinia"

export const useNomeStore = defineStore("nome", {
  state: () => ({
    // estado inicial
  }),
  actions: {
    // metodos
  }
})
```

### Vue Router

Usa `createWebHashHistory()` para compatibilidade com Tauri.

## Tratamento de Erros

### Frontend

```javascript
try {
  const resultado = await invoke("comando", params)
  // sucesso
} catch (e) {
  console.error(e)
  alert("Erro ao ...\n\nDetalhes: " + e)
}
```

### Backend

```rust
.funcao()
.await
.map_err(|e| e.to_string())?;
```

## Padroes de Imports

### Vue

```javascript
import { ref, computed, onMounted, watch } from "vue"
import { useRouter } from "vue-router"
import { defineStore } from "pinia"
```

### Tauri

```javascript
import { invoke } from "@tauri-apps/api/core"
import { open, save } from "@tauri-apps/plugin-dialog"
import { writeTextFile } from "@tauri-apps/plugin-fs"
```

### Stores

```javascript
import { useDatabaseStore } from "../stores/database"
import { useConnectionsStore } from "../stores/connections"
```

## Build Linux via Container

O build Linux usa duas imagens em camadas, orquestradas pelo justfile:

- **Containerfile.base** — Imagem base com Node.js 20, Rust 1.83.0 e dependencias do sistema (WebKitGTK, etc). Reconstruir apenas quando atualizar deps.
- **Containerfile.linux** — Multi-stage: instala deps Node, compila deps Rust (com cache), depois compila o projeto. Extrai binario e bundles para `src-tauri/target/release/`.

```bash
# Build completo (base + projeto)
just build-linux-full

# Apenas rebuild do projeto (imagem base ja existe)
just build-linux

# Apenas rebuild da imagem base
just build-base
```

Container runtime padrao: `podman`. Override: `CONTAINER_CMD=docker just build-linux-full`

## Convencoes de Commits

- Mensagens em portugues ou ingles (seja consistente)
- Formato: `tipo: descricao`
- Tipos: feat, fix, refactor, docs, style, test, chore

Exemplos:
```
feat: adiciona suporte a importacao de dumps
fix: corrige conexao com bancos offline
refactor: reorganiza componentes do sidebar
```

## Notas Importantes

1. **mysqldump obrigatorio**: O app depende do binario `mysqldump` no PATH para exportacao
2. **mysql obrigatorio para import**: O app depende do binario `mysql` no PATH para importacao de dumps
3. **Conexoes salvas em localStorage**: Dados de conexao ficam no navegador/localStorage
4. **Diretorio de dados**: Dumps e settings ficam em `~/.config/munhoz-dumper/` (dumps/ e settings.json)
5. **Nenhum arquivo de configuracao de lint atualmente**: Considere adicionar `.prettierrc` e `rustfmt.toml`
6. **Sem testes**: Projeto nao possui testes automatizados
