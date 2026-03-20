---
name: rust-dev
description: Write idiomatic, production-grade Rust code with strong type safety, performance, and correctness. Use this skill when the user asks to implement Rust modules, commands, structs, async tasks, error handling, or any backend logic in Rust. Avoids over-engineering and newbie patterns; produces code that feels written by a senior Rust developer.
license: MIT
compatibility: opencode
metadata:
  audience: Rust developers
  workflow: backend-development
  origin: project-local
---

Este skill guia a implementação de código Rust idiomático, seguro e de produção — sem padrões de iniciante, sem over-engineering, sem abstrações prematuras.

O usuário descreve o que precisa: um módulo, comando Tauri, struct, função assíncrona, trait, sistema de erros, etc. O skill orienta as decisões de design e implementação.

## Mentalidade Rust

Antes de codificar, pense em:

- **Ownership**: onde os dados vivem, quem os possui, quando são liberados. Prefira mover em vez de clonar. Clone apenas quando o custo é irrelevante ou a alternativa é um lifetime complexo.
- **Erros como valores**: use `Result<T, E>` para tudo que pode falhar. Nunca use `unwrap()` em código de produção — use `?`, `map_err`, ou trate explicitamente. Prefira `String` como erro em comandos Tauri para simplicidade; use tipos de erro próprios em lógica de biblioteca.
- **Tipos expressivos**: o type system é sua primeira linha de defesa. Encode invariants no tipo, não em runtime checks. Prefira `Option<T>` a sentinels como `-1` ou `""`.
- **Zero cost abstractions**: iterators, closures, generics — use-os sem medo. Evite alocações desnecessárias; prefira `&str` a `String`, `&[T]` a `Vec<T>` quando possível.
- **Async com tokio**: use `async/await` para I/O. Para CPU-bound ou blocking I/O, use `spawn_blocking`. Nunca bloqueie dentro de um contexto async.

## Diretrizes de Implementação

### Estrutura e Módulos

- Organize por responsabilidade: `models.rs` para structs de dados, `db.rs` para acesso ao banco, `dump.rs` para lógica de domínio, etc.
- Módulos devem ter uma responsabilidade clara. Se um módulo cresce além de ~200 linhas, considere dividir.
- `pub` apenas o que precisa ser público. Encapsulamento por padrão.

### Structs e Tipos

```rust
// Derive apenas o que for necessário
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Foo {
    pub field: String,
}

// Use newtype para semântica clara
pub struct UserId(u64);
pub struct TableName(String);
```

- Derive `Debug` em quase tudo durante desenvolvimento.
- Derive `Clone` apenas quando você realmente precisa clonar.
- Evite `Default` derivado automaticamente em structs com semântica de negócio — implemente manualmente com valores significativos.

### Tratamento de Erros

```rust
// Em comandos Tauri: Result<T, String> é suficiente
pub async fn meu_comando(...) -> Result<MeuRetorno, String> {
    let resultado = operacao_que_pode_falhar()
        .map_err(|e| format!("Falha ao fazer X: {}", e))?;
    Ok(resultado)
}

// Em lógica interna: propague erros descritivos
fn parse_algo(input: &str) -> Result<Parsed, String> {
    input.parse::<u16>()
        .map_err(|_| format!("'{}' não é uma porta válida", input))
}
```

- Nunca silenciar erros com `let _ = ...` sem comentário explicando por quê.
- Mensagens de erro em português quando expostas ao usuário, em inglês quando são logs internos.

### Async e Concorrência

```rust
// Tarefas paralelas com semáforo para limitar concorrência
let semaphore = Arc::new(Semaphore::new(limite));

let handles: Vec<_> = itens.into_iter().map(|item| {
    let sem = Arc::clone(&semaphore);
    tokio::spawn(async move {
        let _permit = sem.acquire_owned().await?;
        // trabalho aqui — permit liberado ao sair do escopo
        tauri::async_runtime::spawn_blocking(move || {
            trabalho_bloqueante(item)
        }).await?
    })
}).collect();

for handle in handles {
    handle.await??;
}
```

- Prefira `tokio::spawn` para tarefas I/O-bound e `spawn_blocking` para CPU-bound ou syscalls bloqueantes.
- Use `Arc<Semaphore>` para limitar paralelismo. O limite padrão razoável para I/O externo (ex: mysqldump) é 4–8.
- Evite `Mutex` quando possível. Prefira canais (`tokio::sync::mpsc`) para comunicação entre tasks.

### Comandos Tauri

```rust
#[tauri::command]
async fn meu_comando(
    app: tauri::AppHandle,   // só se precisar emitir eventos ou acessar estado
    conn: String,
    dados: Vec<MinhaStruct>,
) -> Result<MeuRetorno, String> {
    // ...
}
```

- Parâmetros simples por valor. Structs complexas serializadas automaticamente via `serde`.
- Use `app.emit("nome-evento", payload)` com `use tauri::Emitter;` em escopo.
- Registre todos os comandos em `generate_handler![]` — o compilador não avisa se esquecer.

### Performance e Alocação

- Leia arquivos grandes com `BufReader` + `io::copy` — nunca carregue tudo em memória.
- Para strings de saída acumuladas, prefira `String::with_capacity(estimativa)`.
- Evite `.clone()` em hot paths. Se clonando frequentemente, reconsidere a estrutura de ownership.

### Padrões a Evitar

```rust
// ❌ Nunca em produção
value.unwrap()
value.expect("mensagem genérica")

// ❌ Clone desnecessário
fn processa(s: String) -> String { ... }  // prefira &str se não precisar de ownership

// ❌ Abstração prematura
trait GenericProcessor<T: Processable + Serializable + Debug> { ... }
// para um único caso de uso

// ❌ Ignorar o compilador
#[allow(unused)]  // sem motivo documentado
```

### Padrões Preferidos

```rust
// ✅ Propagação limpa de erro
let x = operacao()?;

// ✅ Iteradores em vez de loops imperativos
let nomes: Vec<String> = tabelas.iter().map(|t| t.name.clone()).collect();

// ✅ Pattern matching expressivo
match resultado {
    Ok(v) if v.is_empty() => Err("resultado vazio".to_string()),
    Ok(v) => Ok(v),
    Err(e) => Err(format!("falha: {}", e)),
}

// ✅ Configuração com Default bem definido
impl Default for AppSettings {
    fn default() -> Self {
        Self { max_parallel_dumps: 5 }
    }
}
```

## Checklist antes de Finalizar

- [ ] Nenhum `unwrap()` ou `expect()` em código de produção
- [ ] Erros expostos ao usuário estão em português e são descritivos
- [ ] Funções `async` não chamam código bloqueante diretamente
- [ ] Structs derivam apenas o que é necessário
- [ ] Módulos têm responsabilidade única e clara
- [ ] `cargo build` compila sem warnings
