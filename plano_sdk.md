# Plano: Otimização de Build e Containerfiles

## Contexto

O projeto Dumper (Tauri 2 + Vue 3) usa dois Containerfiles para build Linux via container. O principal gargalo é que o estágio `rust-deps` apenas faz `cargo fetch` (download dos crates) mas **não compila as dependências**, desperdiçando a oportunidade de cache mais valiosa — a compilação Rust leva 5-10min e é refeita a cada build mesmo sem mudança em `Cargo.toml`. Além disso, há melhorias de DX no justfile e limpeza geral.

---

## Mudanças (em ordem de impacto)

### 1. `.dockerignore` — Reduzir contexto de build
**Arquivo:** `/home/barboza/Repos/Munhoz/dumper/.dockerignore`

Adicionar exclusões de arquivos desnecessários no container:
```
Containerfile*
justfile
.opencode/
.claude/
AGENTS.md
plano.md
README.md
```

### 2. `Containerfile.linux` — Cache de compilação Rust (maior impacto)
**Arquivo:** `/home/barboza/Repos/Munhoz/dumper/Containerfile.linux`

**2a.** No estágio `rust-deps`, trocar `cargo fetch` por `cargo build --release --locked` com sources dummy. Isso compila todas as dependências e cacheia o resultado — somente o código do projeto recompila quando só os `.rs` mudam.

Também copiar `tauri.conf.json`, `capabilities/` e `icons/` nesse estágio para que o `build.rs` (tauri_build) consiga executar sem erros.

Após o build, remover fingerprints e artefatos do crate `dumper`/`dumper_lib` para forçar recompilação apenas do código do projeto:
```dockerfile
RUN rm -rf src-tauri/target/release/.fingerprint/dumper-* \
           src-tauri/target/release/.fingerprint/dumper_lib-* \
           src-tauri/target/release/deps/dumper* \
           src-tauri/target/release/deps/libdumper* \
           src-tauri/target/release/dumper*
```

**2b.** Adicionar `ENV CARGO_INCREMENTAL=0` — incremental compilation é inútil em containers e gasta espaço.

**2c.** Adicionar `--locked` em todos os comandos cargo para builds reprodutíveis.

**2d.** Simplificar o estágio `builder` — como `tauri.conf.json`, `capabilities/` e `icons/` já estão no `rust-deps`, o builder só precisa copiar os fontes:
```dockerfile
FROM rust-deps AS builder
COPY src-tauri/src/ ./src-tauri/src/
COPY index.html vite.config.js ./
COPY public/ ./public/
COPY src/ ./src/
RUN npm run tauri build
```

### 3. `Containerfile.base` — Instalação mais limpa
**Arquivo:** `/home/barboza/Repos/Munhoz/dumper/Containerfile.base`

**3a.** Trocar instalação do Node.js via `curl | bash` (nodesource) por `COPY --from=node:20-bookworm-slim` — mais seguro e reprodutível.

**3b.** Remover `wget` da lista apt (era só para nodesource).

**3c.** Remover `default-mysql-client` — não é usado no build, economiza ~30MB.

**3d.** Pinnar versão do Rust (ex: `--default-toolchain 1.83.0`) e usar `--profile minimal` em vez de `--profile default` (não precisa de rustfmt/clippy no container de build).

### 4. `justfile` — Robustez e DX
**Arquivo:** `/home/barboza/Repos/Munhoz/dumper/justfile`

**4a.** Trocar o recipe `_podman_or_docker` por uma variável com override via env:
```just
container_cmd := env("CONTAINER_CMD", "podman")
```

**4b.** No `build-linux`, adicionar cleanup do container extract antes de criar (falha se já existe de build anterior):
```just
build-linux:
    #!/usr/bin/env bash
    set -e
    {{container_cmd}} rm {{project_name}}-extract 2>/dev/null || true
    time {{container_cmd}} build -f Containerfile.linux -t {{project_name}}-builder:latest .
    ...
```

**4c.** Adicionar `time` nos comandos de build para feedback de duração.

**4d.** Adicionar `mkdir -p src-tauri/target/release` antes do `cp` para evitar erro se o diretório não existir.

**4e.** Adicionar recipe `clean-containers` para limpar imagens/containers de build.

---

## Arquivos a modificar

1. `.dockerignore` — adicionar exclusões
2. `Containerfile.linux` — cache de deps compiladas, --locked, CARGO_INCREMENTAL=0
3. `Containerfile.base` — Node via multi-stage, remover mysql-client e wget, pinnar Rust
4. `justfile` — variável container_cmd, cleanup, timing, mkdir -p

## Verificação

1. `just build-base` — deve buildar sem erros, imagem menor (sem mysql-client/wget)
2. `just build-linux` — deve buildar com sucesso, segundo build (sem mudança em Cargo.toml) deve ser significativamente mais rápido
3. Verificar que os artefatos são extraídos corretamente em `src-tauri/target/release/`
4. Testar `CONTAINER_CMD=docker just build-linux` para validar override
