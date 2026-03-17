<script setup>

import { ref, computed, onMounted, watch } from "vue"
import { invoke } from "@tauri-apps/api/core"
import { open } from "@tauri-apps/plugin-dialog"

import { useDatabaseStore } from "../stores/database"
import DumpExport from "../components/DumpExport.vue"

const store = useDatabaseStore()

const filter = ref("")

const loading = ref(false)
const importing = ref(false)
const importCancelled = ref(false)
const importFileName = ref("")

const sortKey = ref("name")
const sortAsc = ref(true)

const filteredTables = computed(() => {

  if (!store.tables)
    return []

  const term = filter.value.toLowerCase()

  const filtered = store.tables.filter(t =>
    t.name.toLowerCase().includes(term)
  )

  const sorted = [...filtered].sort((a, b) => {

    if (sortKey.value === "size") {

      return sortAsc.value
        ? a.size - b.size
        : b.size - a.size

    }

    return sortAsc.value
      ? a.name.localeCompare(b.name)
      : b.name.localeCompare(a.name)

  })

  return sorted

})

const selectedSummary = computed(() => {

  if (!store.tables || !store.tables.length)
    return { count: 0, bytes: 0 }

  const selected = store.tables.filter(
    t => t.structure || t.content || t.drop
  )

  const count = selected.length
  const bytes = selected.reduce((sum, t) => sum + (t.size || 0), 0)

  return { count, bytes }

})

async function loadDatabases(){

  if (!store.connection)
    return

  loading.value = true

  try{

    const dbs = await invoke("databases",{
      conn: store.connection.url || store.connection
    })

    store.databases = dbs

  }catch(e){

    console.error(e)

  }

  loading.value = false

}

async function loadTables(){

  if(!store.selectedDatabase || !store.connection)
    return

  loading.value = true

  try{

    const tables = await invoke("tables_with_size",{

      conn: store.connection.url || store.connection,

      database: store.selectedDatabase

    })

    store.tables = tables.map(t=>({

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

async function importDump(){

  if (!store.connection || !store.selectedDatabase)
    return

  const path = await open({
    multiple: false,
    directory: false,
    filters: [{ name: "SQL", extensions: ["sql"] }],
    title: "Selecionar arquivo de dump"
  })

  if (!path)
    return

  importing.value = true
  importCancelled.value = false
  importFileName.value = typeof path === "string" ? path.split(/[\\/]/).pop() || "" : ""

  try{

    await invoke("import_dump", {
      conn: store.connection.url || store.connection,
      database: store.selectedDatabase,
      path
    })

    if (!importCancelled.value) {
      alert("Dump importado com sucesso.")
      loadTables()
    }

  }catch(e){

    console.error(e)
    alert("Erro ao importar dump.\n\nDetalhes: " + e)

  }

  importing.value = false

}

function cancelImport(){

  importCancelled.value = true
  importing.value = false

}

function toggleSort(key){

  if (sortKey.value === key) {

    sortAsc.value = !sortAsc.value

  } else {

    sortKey.value = key
    sortAsc.value = true

  }

}

function toggleAll(field){

  const visible = filteredTables.value

  if (!visible.length)
    return

  const allVisibleChecked = visible.every(t => t[field])

  visible.forEach(t => {
    t[field] = !allVisibleChecked
  })

}

function formatSize(bytes){

 if(bytes < 1024)
  return bytes + " B"

 if(bytes < 1024*1024)
  return (bytes/1024).toFixed(1) + " KB"

 if(bytes < 1024*1024*1024)
  return (bytes/1024/1024).toFixed(2) + " MB"

 return (bytes/1024/1024/1024).toFixed(2) + " GB"

}

onMounted(()=>{

  loadDatabases()

})

watch(
  () => store.connection,
  () => {
    store.databases = []
    store.selectedDatabase = null
    store.tables = []
    loadDatabases()
  }
)

</script>

<template>

  <div class="database-page">
    <div
      v-if="store.selectedDatabase"
      class="database-controls-row"
    >
      <input
        class="field-input database-search"
        v-model="filter"
        placeholder="Buscar tabelas..."
      >

      <button
        type="button"
        class="btn btn--ghost"
        title="Atualizar lista de tabelas"
        :disabled="!store.connection || !store.selectedDatabase || loading"
        @click="loadTables"
      >
        <i class="ri-refresh-line"></i>
        <span>Atualizar tabelas</span>
      </button>

      <DumpExport
        :tables="filteredTables"
      />

      <button
        type="button"
        class="btn btn--primary"
        title="Importar arquivo .sql no banco selecionado"
        :disabled="!store.connection || !store.selectedDatabase || loading || importing"
        @click="importDump"
      >
        <i class="ri-download-2-line"></i>
        <span>{{ importing ? "Importando..." : "Importar" }}</span>
      </button>
    </div>

    <div class="database-main">
      <div
        v-if="!store.selectedDatabase"
        class="database-dbs-container"
      >
        <div class="database-dbs-header">
          <div class="database-dbs-title-group">
            <h2 class="database-dbs-title">
              {{ store.connection?.name || 'Conexão sem nome' }}
            </h2>
            <p class="database-dbs-subtitle">
              {{ store.databases.length || 0 }} databases disponíveis
            </p>
          </div>
        </div>

        <div class="database-dbs-grid">
          <div
            v-for="db in store.databases"
            :key="db"
            class="database-card"
            :style="{ '--accent': store.connection?.color || '#3a7afe' }"
            @click="store.selectedDatabase = db; loadTables()"
          >
            <div class="database-card-badge">
              <span class="database-status-badge">
                Database
              </span>
            </div>
            <div class="database-card-body">
              <div class="database-card-name">
                {{ db }}
              </div>
            </div>
          </div>
        </div>
      </div>

      <div v-else class="database-table-area">
        <div v-if="loading" class="loading">
          <div class="progress-bar">
            <div class="progress-bar-inner"></div>
          </div>
          <span>Carregando tabelas...</span>
        </div>

        <div
          v-if="!loading"
          class="table-container"
        >
          <table>
            <thead>
              <tr>
                <th
                  class="sortable"
                  @click="toggleSort('name')"
                >
                  Tabela
                </th>
                <th
                  class="sortable"
                  @click="toggleSort('size')"
                >
                  Tamanho
                </th>
                <th class="col-action">
                  <input
                    type="checkbox"
                    title="Structure"
                    :checked="filteredTables.length && filteredTables.every(t => t.structure)"
                    @change="toggleAll('structure')"
                  >
                  S
                </th>
                <th class="col-action">
                  <input
                    type="checkbox"
                    title="Content"
                    :checked="filteredTables.length && filteredTables.every(t => t.content)"
                    @change="toggleAll('content')"
                  >
                  C
                </th>
                <th class="col-action">
                  <input
                    type="checkbox"
                    title="Drop"
                    :checked="filteredTables.length && filteredTables.every(t => t.drop)"
                    @change="toggleAll('drop')"
                  >
                  D
                </th>
              </tr>
            </thead>

            <tbody>
              <tr
                v-for="t in filteredTables"
                :key="t.name"
              >
                <td>{{ t.name }}</td>
                <td>{{ formatSize(t.size) }}</td>
                <td class="col-action">
                  <input
                    type="checkbox"
                    v-model="t.structure"
                  >
                </td>
                <td class="col-action">
                  <input
                    type="checkbox"
                    v-model="t.content"
                  >
                </td>
                <td class="col-action">
                  <input
                    type="checkbox"
                    v-model="t.drop"
                  >
                </td>
              </tr>
            </tbody>
          </table>
        </div>

        <div
          v-if="store.tables && store.tables.length"
          class="summary"
        >
          <template v-if="selectedSummary.count">
            {{ selectedSummary.count }} tabelas:
            {{ formatSize(selectedSummary.bytes) }}
          </template>
          <template v-else>
            Nenhuma tabela selecionada
          </template>
        </div>
      </div>
    </div>
  </div>

  <div
    v-if="importing"
    class="import-floating-toolbar"
  >
    <div class="import-floating-main">
      <div class="import-floating-info">
        <span class="import-floating-label">
          Importando:
        </span>
        <span class="import-floating-table">
          {{ importFileName || 'Processando arquivo...' }}
        </span>
      </div>

      <div class="import-floating-progress">
        <div class="import-floating-progress-bar">
          <div class="import-floating-progress-bar-inner" />
        </div>
        <span class="import-floating-status">
          Aguardando conclusão...
        </span>
      </div>
    </div>

    <button
      type="button"
      class="import-floating-cancel"
      title="Cancelar importação"
      @click="cancelImport"
    >
      <i class="ri-close-line"></i>
    </button>
  </div>

</template>

<style scoped>

.database-page{
  display:flex;
  flex-direction:column;
  height:100%;
  min-height:0;
}

.database-controls-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto auto auto;
  gap: 10px;
  margin-bottom: 16px;
}

.database-search {
  width: 100%;
}

.database-main {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.database-dbs-container {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}

.database-dbs-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 16px;
}

.database-dbs-title-group {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.database-dbs-title {
  margin: 0;
  font-size: 16px;
  font-weight: 500;
}

.database-dbs-subtitle {
  margin: 0;
  font-size: 12px;
  color: #9ca3af;
}

.database-dbs-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  grid-auto-rows: minmax(120px, 1fr);
  gap: 16px;
  align-content: flex-start;
}

.database-card {
  position: relative;
  border-radius: 18px;
  padding: 14px 14px 12px;
  background:
    radial-gradient(
      circle at top left,
      color-mix(in srgb, var(--accent, #3a7afe) 32%, transparent) 0,
      transparent 60%
    ),
    radial-gradient(
      circle at bottom right,
      color-mix(in srgb, var(--accent, #3a7afe) 18%, transparent) 0,
      transparent 65%
    ),
    linear-gradient(
      135deg,
      rgba(15, 23, 42, 0.96),
      rgba(15, 23, 42, 1)
    );
  box-shadow:
    0 18px 40px rgba(0, 0, 0, 0.75),
    0 0 0 1px rgba(39, 39, 42, 0.8);
  cursor: pointer;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  opacity: 1;
  transition: transform 0.12s ease, box-shadow 0.12s ease,
    opacity 0.12s ease, border-color 0.12s ease, background 0.12s ease;
  border: 1px solid color-mix(in srgb, var(--accent, #3a7afe) 70%, #020617 30%);
}

.database-card::before {
  content: "";
  position: absolute;
  inset: 0;
  border-radius: inherit;
  background: radial-gradient(
    circle at top left,
    color-mix(in srgb, var(--accent, #3a7afe) 35%, transparent),
    transparent
  );
  opacity: 0.5;
  pointer-events: none;
}

.database-card:hover {
  transform: translateY(-2px);
  box-shadow:
    0 24px 60px rgba(0, 0, 0, 0.85),
    0 0 0 1px rgba(37, 99, 235, 0.8);
}

.database-card-badge {
  display: flex;
  align-items: center;
}

.database-status-badge {
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.14em;
  padding: 3px 8px;
  border-radius: 999px;
  border: 1px solid #27272a;
  color: #e5e7eb;
  background: rgba(31, 41, 55, 0.9);
}

.database-card-body {
  display: flex;
  flex-direction: column;
  justify-content: flex-end;
  flex: 1;
  padding-top: 12px;
}

.database-card-name {
  font-size: 18px;
  font-weight: 600;
  color: #f9fafb;
  word-break: break-all;
}

.database-table-area {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.database-empty {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
}

.database-empty-card {
  padding: 28px 24px;
  border-radius: 18px;
  border: 1px dashed #27272a;
  background: radial-gradient(circle at top left, #020617, #020617);
  max-width: 420px;
}

.database-empty-title {
  margin: 0 0 8px;
  font-size: 18px;
  font-weight: 500;
}

.database-empty-text {
  margin: 0;
  font-size: 13px;
  color: #9ca3af;
}

.loading{
  display:flex;
  align-items:center;
  gap:8px;
  font-size:14px;
  color:#555;
}

.progress-bar{
  width:120px;
  height:4px;
  background:#e0e0e0;
  border-radius:999px;
  overflow:hidden;
}

.progress-bar-inner{
  width:40%;
  height:100%;
  background:#3a7afe;
  border-radius:999px;
  animation: progress-pulse 1s infinite ease-in-out;
}

.summary{
  margin-top:8px;
  font-size:13px;
  color:#9ca3af;
}

.summary--inline{
  margin-top:0;
  margin-left:auto;
}

.table-container{
  flex:1;
  min-height:0;
  overflow-y:auto;
  border-radius:12px;
  border: 1px solid #111827;
}

thead {
  position: sticky;
  top: 0;
  z-index: 1;
}

.col-action{
  width:60px;
  text-align:center;
  white-space:nowrap;
}

@keyframes progress-pulse{
  0%{ transform:translateX(-100%); }
  50%{ transform:translateX(0%); }
  100%{ transform:translateX(100%); }
}

.import-floating-toolbar{
  position: fixed;
  left: 50%;
  bottom: 16px;
  transform: translateX(-50%);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 14px;
  min-width: 420px;
  max-width: 480px;
  border-radius: 999px;
  background: rgba(15, 23, 42, 0.95);
  border: 1px solid #27272a;
  box-shadow:
    0 18px 40px rgba(0, 0, 0, 0.8),
    0 0 0 1px rgba(15, 23, 42, 0.9);
  color: #e5e7eb;
  z-index: 50;
}

.import-floating-main{
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.import-floating-info{
  display: flex;
  flex-direction: row;
  gap: 8px;
  align-items: baseline;
}

.import-floating-label{
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.12em;
  color: #9ca3af;
}

.import-floating-table{
  font-size: 13px;
  font-weight: 500;
  max-width: 220px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.import-floating-progress{
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
}

.import-floating-progress-bar{
  flex: 1;
  height: 4px;
  background: linear-gradient(
    90deg,
    rgba(15, 23, 42, 0.9),
    rgba(15, 23, 42, 0.7)
  );
  border-radius:999px;
  overflow:hidden;
}

.import-floating-progress-bar-inner{
  width: 40%;
  height:100%;
  background:#3a7afe;
  border-radius:999px;
  animation: progress-pulse 1s infinite ease-in-out;
}

.import-floating-status{
  font-size: 12px;
  color: #e5e7eb;
}

.import-floating-cancel{
  border-radius: 999px;
  border: 1px solid #4b5563;
  background: #020617;
  color: #e5e7eb;
  padding: 4px;
  font-size: 14px;
  line-height: 1;
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

</style>