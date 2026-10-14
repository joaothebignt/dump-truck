<script setup>
import { computed, onMounted, ref, watch } from "vue"
import { RouterView, useRouter } from "vue-router"
import { invoke } from "@tauri-apps/api/core"
import { useDatabaseStore } from "./stores/database"
import {
  useExportDefaultsStore,
  selectionFromExportDefaults
} from "./stores/exportDefaults"

const dbStore = useDatabaseStore()
const exportDefaults = useExportDefaultsStore()
const router = useRouter()

const loading = ref(false)

const hasConnection = computed(() => !!dbStore.connection)

const connectionLabel = computed(() => {
  const conn = dbStore.connection || {}
  if (conn.host) {
    return conn.port ? `${conn.host}:${conn.port}` : conn.host
  }
  if (typeof conn.url === "string" && conn.url) {
    try {
      const parsed = new URL(conn.url)
      return parsed.port ? `${parsed.hostname}:${parsed.port}` : parsed.hostname
    } catch {
      return conn.url
    }
  }
  return "Conectado"
})

async function loadDatabases(){
  if (!dbStore.connection)
    return

  loading.value = true

  try{
    const dbs = await invoke("databases",{
      conn: dbStore.connection.url || dbStore.connection
    })
    dbStore.databases = dbs
  }catch(e){
    console.error(e)
  }

  loading.value = false
}

async function loadTables(){
  if (!dbStore.selectedDatabase || !dbStore.connection)
    return

  loading.value = true

  try{
    const tables = await invoke("tables_with_size",{
      conn: dbStore.connection.url || dbStore.connection,
      database: dbStore.selectedDatabase
    })

    dbStore.tables = tables.map(t => {
      const sel = selectionFromExportDefaults(t.name, exportDefaults.rules)
      return {
        name: t.name,
        size: t.size,
        structure: sel.structure,
        content: sel.content,
        drop: sel.drop
      }
    })
  }catch(e){
    console.error(e)
  }

  loading.value = false
}

function handleDatabaseChange(){
  loadTables()
}

function refreshDatabases(){
  loadDatabases()
}

function goHome(){
  dbStore.clearConnection()
  router.push("/")
}

function clearDatabase(){
  dbStore.selectedDatabase = null
  dbStore.tables = []
}

onMounted(() => {
  if (dbStore.connection) {
    loadDatabases()
  }
})

watch(
  () => dbStore.connection,
  () => {
    dbStore.databases = []
    dbStore.selectedDatabase = null
    dbStore.tables = []
    if (dbStore.connection) {
      loadDatabases()
    }
  }
)
</script>

<template>
  <div class="layout">
    <header class="topbar">
      <div class="topbar-left">
        <div class="logo">Dump Truck</div>
      </div>

      <div class="topbar-center" v-if="hasConnection">
        <button
          type="button"
          class="btn btn--ghost btn--icon topbar-home"
          title="Voltar para conexões"
          :style="{ '--btn-accent': dbStore.connection?.color || '#3a7afe' }"
          @click="goHome"
        >
          <i class="ri-home-line"></i>
        </button>

        <div
          class="connection-tab"
          :style="{ '--accent': dbStore.connection?.color || '#3a7afe' }"
        >
          <div class="connection-main">
            <span class="connection-name">
              {{ dbStore.connection?.name || "Conexão ativa" }}
            </span>
          </div>
          <div class="connection-db-select">
            <span class="connection-db-label">
              Database
            </span>
            <span class="connection-db-value">
              {{ dbStore.selectedDatabase || "Nenhum selecionado" }}
            </span>
            <button
              v-if="dbStore.selectedDatabase"
              type="button"
              class="connection-db-clear"
              title="Fechar database atual"
              @click.stop="clearDatabase"
            >
              <i class="ri-close-line"></i>
            </button>
          </div>
        </div>
      </div>

      <div class="topbar-right">
        <span
          v-if="hasConnection"
          class="topbar-status topbar-status--online"
          :style="{ '--accent': dbStore.connection?.color || '#3a7afe' }"
        >
          {{ connectionLabel }}
        </span>
        <span v-else class="topbar-status">
          Nenhuma conexão ativa
        </span>
      </div>
    </header>

    <main class="main">
      <section class="content">
        <div class="content-inner">
          <RouterView />
        </div>
      </section>
    </main>
  </div>
</template>

<style>
html,
body,
#app {
  height: 100%;
  margin: 0;
  font-family: system-ui, -apple-system, sans-serif;
  background: #000;
  color: #f5f5f5;
}

*,
*::before,
*::after {
  box-sizing: border-box;
}

.layout {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: radial-gradient(circle at top, #111 0, #000 55%);
}

.topbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 20px;
  border-bottom: 1px solid #151515;
  background: rgba(0, 0, 0, 0.9);
  backdrop-filter: blur(12px);
  position: relative;
}

.topbar-left,
.topbar-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

.topbar-center {
  position: absolute;
  left: 50%;
  transform: translateX(-50%);
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
}

.logo {
  font-size: 16px;
  letter-spacing: 0.16em;
  text-transform: uppercase;
  font-weight: 600;
  color: #f5f5f5;
}

.connection-tab {
  display: inline-flex;
  align-items: center;
  gap: 10px;
  padding: 6px 14px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--accent, #3a7afe) 14%, rgba(15, 23, 42, 0.92));
  box-shadow: 0 0 0 1px color-mix(in srgb, var(--accent, #3a7afe) 55%, transparent);
}

.connection-main {
  display: flex;
  align-items: center;
  padding-right: 8px;
  border-right: 1px solid rgba(148, 163, 184, 0.3);
}

.connection-name {
  font-size: 13px;
  font-weight: 500;
  color: #e5e7eb;
}

.connection-db-select {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding-left: 6px;
}

.connection-db-label {
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.1em;
  color: #9ca3af;
}

.connection-db-value {
  font-size: 13px;
  color: #f9fafb;
}

.connection-db-clear {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  padding: 0;
  border-radius: 999px;
  border: 1px solid color-mix(in srgb, var(--accent, #3a7afe) 45%, #374151 55%);
  background: color-mix(in srgb, var(--accent, #3a7afe) 12%, transparent);
  color: #e5e7eb;
  font-size: 13px;
  line-height: 1;
  cursor: pointer;
}

.connection-db-clear:hover {
  background: color-mix(in srgb, var(--accent, #3a7afe) 22%, transparent);
}

.topbar-home {
  flex-shrink: 0;
}

.field-select--pill {
  min-width: 160px;
  padding-block: 5px;
  padding-right: 26px;
  font-size: 12px;
  border-radius: 999px;
  background:
    radial-gradient(circle at top left, #020617, #020617);
  position: relative;
  appearance: none;
  -webkit-appearance: none;
}

.field-select--pill::-ms-expand {
  display: none;
}

.field-select--pill:focus {
  border-color: #3a7afe;
  box-shadow: 0 0 0 1px rgba(58, 122, 254, 0.7);
}

.field-select--pill {
  background-image: linear-gradient(45deg, #6b7280 0%, #6b7280 100%);
  background-position: right 10px center;
  background-repeat: no-repeat;
  background-size: 1px 14px;
}

.connection-refresh {
  font-size: 11px;
  padding-inline: 10px;
}

.topbar-status {
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.12em;
  padding: 4px 10px;
  border-radius: 999px;
  border: 1px solid #27272a;
  color: #a1a1aa;
  background: rgba(9, 9, 11, 0.8);
}

.topbar-status--online {
  border-color: color-mix(in srgb, var(--accent, #3a7afe) 75%, #27272a 25%);
  color: color-mix(in srgb, var(--accent, #3a7afe) 35%, #e5e7eb 65%);
  background: color-mix(in srgb, var(--accent, #3a7afe) 16%, transparent);
}

.icon {
  display: inline-block;
  line-height: 1;
}

.main {
  flex: 1;
  display: flex;
  overflow: hidden;
}

.content {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0;
  padding: 20px 24px 0;
}

.content-inner {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

/* UI base: inputs, botões, selects, checkboxes */
button,
input,
select,
textarea {
  font-family: inherit;
}

.btn {
  --btn-accent: #3a7afe;
  border-radius: 999px;
  border: 1px solid #27272a;
  background: #050816;
  color: #e5e7eb;
  padding: 8px 14px;
  font-size: 13px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  cursor: pointer;
  transition: background 0.15s ease, border-color 0.15s ease,
    transform 0.05s ease, box-shadow 0.15s ease, color 0.15s ease;
}

.btn--primary {
  border-color: var(--btn-accent);
  background: linear-gradient(
    135deg,
    color-mix(in srgb, var(--btn-accent) 82%, #020617 18%),
    color-mix(in srgb, var(--btn-accent) 92%, #38bdf8 8%)
  );
  color: #f9fafb;
  box-shadow: 0 10px 30px color-mix(in srgb, var(--btn-accent) 65%, transparent);
}

.btn--ghost {
  border-color: #27272a;
  background: #020617;
}

.btn--icon {
  padding-inline: 10px;
}

.btn:hover:not(:disabled) {
  background: #020617;
  border-color: var(--btn-accent);
  box-shadow: 0 12px 40px color-mix(in srgb, var(--btn-accent) 70%, transparent);
  transform: translateY(-0.5px);
}

.btn--primary:hover:not(:disabled) {
  background: linear-gradient(
    135deg,
    color-mix(in srgb, var(--btn-accent) 90%, #020617 10%),
    color-mix(in srgb, var(--btn-accent) 96%, #38bdf8 4%)
  );
}

.btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
  box-shadow: none;
}

.field-input,
.field-select {
  width: 100%;
  border-radius: 10px;
  border: 1px solid #27272a;
  background: radial-gradient(circle at top left, #020617, #020617);
  color: #e5e7eb;
  padding: 8px 10px;
  font-size: 13px;
  outline: none;
  transition: border-color 0.15s ease, box-shadow 0.15s ease,
    background 0.15s ease;
}

.field-input::placeholder {
  color: #6b7280;
}

.field-input:focus,
.field-select:focus {
  border-color: #3a7afe;
  box-shadow: 0 0 0 1px rgba(58, 122, 254, 0.7);
}

input[type="checkbox"] {
  width: 14px;
  height: 14px;
  border-radius: 4px;
  border: 1px solid #3f3f46;
  background: #020617;
  appearance: none;
  cursor: pointer;
  position: relative;
  display: inline-block;
}

input[type="checkbox"]:checked {
  border-color: #3a7afe;
  background: #1d4ed8;
}

input[type="checkbox"]:checked::after {
  content: "";
  position: absolute;
  inset: 2px;
  border-radius: 2px;
  background: #e5e7eb;
}

table {
  width: 100%;
  border-collapse: collapse;
  background: radial-gradient(circle at top left, #020617, #020617);
  color: #e5e7eb;
}

thead {
  background: rgba(15, 23, 42, 0.9);
}

th,
td {
  border-bottom: 1px solid #111827;
  padding: 8px 10px;
  font-size: 13px;
}

th {
  text-align: left;
  font-weight: 500;
  color: #9ca3af;
}

tr:hover td {
  background: radial-gradient(circle at top left, #020617, #020617);
}
</style>