<script setup>
import { computed, onMounted, ref, watch } from "vue"
import { RouterView, useRouter } from "vue-router"
import { invoke } from "@tauri-apps/api/core"
import { useDatabaseStore } from "./stores/database"

const dbStore = useDatabaseStore()
const router = useRouter()

const loading = ref(false)

const hasConnection = computed(() => !!dbStore.connection)

const connectionLabel = computed(() => {
  debugger
  const conn = dbStore.connection || {}
  if (conn.name && conn.host) {
    return `${conn.name} – ${conn.host}`
  }
  return conn.name || conn.host || "Conectado"
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

    dbStore.tables = tables.map(t=>({
      name: t.name,
      size: t.size,
      structure: true,
      content: true,
      drop: true
    }))
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

function closeConnection(){
  dbStore.clearConnection()
  dbStore.selectedDatabase = null
  dbStore.tables = []
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
        <div class="logo">DB Dumper</div>
      </div>

      <div class="topbar-center" v-if="hasConnection">
        <div class="connection-tab">
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
            <span
              v-if="dbStore.selectedDatabase"
              class="connection-db-clear"
              title="Fechar database atual"
              @click.stop="clearDatabase"
            >
              <i class="ri-close-line"></i>
            </span>
          </div>
        </div>
      </div>

      <div class="topbar-right">
        <span
          v-if="hasConnection"
          class="topbar-status topbar-status--online"
        >
          {{ connectionLabel }}
        </span>
        <span v-else class="topbar-status">
          Nenhuma conexão ativa
        </span>
        <button
          v-if="hasConnection"
          class="btn btn--ghost btn--icon topbar-close"
          type="button"
          @click="closeConnection"
        >
          <i class="ri-close-line"></i>
        </button>
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
  justify-content: center;
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
  background: rgba(15, 23, 42, 0.8);
  box-shadow: 0 0 0 1px rgba(148, 163, 184, 0.25);
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
  font-size: 12px;
  padding-inline: 8px;
  padding-block: 4px;
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
  border-color: #3a7afe;
  color: #e5edff;
  background: rgba(58, 122, 254, 0.1);
}

.topbar-close {
  font-size: 11px;
  padding-inline: 12px;
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
  padding: 20px 24px 24px;
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
  border-color: #3a7afe;
  background: linear-gradient(135deg, #1d4ed8, #3b82f6);
  color: #f9fafb;
  box-shadow: 0 10px 30px rgba(37, 99, 235, 0.45);
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
  border-color: #3a7afe;
  box-shadow: 0 12px 40px rgba(37, 99, 235, 0.5);
  transform: translateY(-0.5px);
}

.btn--primary:hover:not(:disabled) {
  background: linear-gradient(135deg, #2563eb, #60a5fa);
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