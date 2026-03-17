# Justfile para build do Dumper (Tauri + Vue 3)
# Uso: just <comando>
# Override do container runtime: CONTAINER_CMD=docker just build-linux

project_name := "dumper"
container_cmd := env("CONTAINER_CMD", "podman")

default:
    @just --list

# Instala dependências Node.js
install:
    npm install

# Inicia o ambiente de desenvolvimento (Tauri + Vite)
dev: install
    npm run tauri dev

# Build completo do app (nativo, para o SO atual)
build:
    npm run tauri build

# Build apenas do frontend (Vite)
build-frontend:
    npm run build

# Cria a imagem base do container (Node + Rust + deps do sistema)
build-base:
    time {{container_cmd}} build -f Containerfile.base -t dumper-base:latest .

# Build Linux via container e extrai os artefatos para src-tauri/target/release/
build-linux:
    #!/usr/bin/env bash
    set -e
    {{container_cmd}} rm {{project_name}}-extract 2>/dev/null || true
    time {{container_cmd}} build -f Containerfile.linux -t {{project_name}}-builder:latest .
    {{container_cmd}} create --name {{project_name}}-extract {{project_name}}-builder:latest
    mkdir -p src-tauri/target/release
    {{container_cmd}} cp {{project_name}}-extract:/bundle/. ./src-tauri/target/release/bundle/
    {{container_cmd}} cp {{project_name}}-extract:/dumper ./src-tauri/target/release/dumper
    {{container_cmd}} rm {{project_name}}-extract

# Build Linux completo: imagem base + build do projeto
build-linux-full: build-base build-linux

# Build release sem gerar bundles (.deb, .AppImage, etc)
build-release:
    npm run tauri build -- --bundles none

# Formata código Rust e Vue/JS
fmt:
    cd src-tauri && cargo fmt
    npx prettier --write "src/**/*.{js,vue}"

# Verifica formatação sem modificar arquivos
fmt-check:
    cd src-tauri && cargo fmt --check
    npx prettier --check "src/**/*.{js,vue}"

# Executa clippy no código Rust
lint-rust:
    cd src-tauri && cargo clippy --all-targets --all-features -- -D warnings

# Verifica formatação e executa lints
lint: fmt-check lint-rust

# Remove artefatos de build (target/, node_modules/, dist/)
clean:
    cd src-tauri && cargo clean
    rm -rf node_modules dist

# Remove artefatos de build e lock files
clean-all: clean
    rm -f package-lock.json
    rm -f src-tauri/Cargo.lock

# Remove imagens e containers de build
clean-containers:
    {{container_cmd}} rmi {{project_name}}-builder:latest 2>/dev/null || true
    {{container_cmd}} rmi {{project_name}}-base:latest 2>/dev/null || true
    {{container_cmd}} rm {{project_name}}-extract 2>/dev/null || true

# Pipeline de CI: install, lint e build
ci: install lint build

# Pipeline de release: install, fmt, lint e build
release: install fmt lint build
