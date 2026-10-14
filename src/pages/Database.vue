<script setup>

import { ref, computed, onMounted, watch } from "vue"
import { invoke } from "@tauri-apps/api/core"
import { open } from "@tauri-apps/plugin-dialog"

import { useDatabaseStore } from "../stores/database"
import {
  useExportDefaultsStore,
  selectionFromExportDefaults
} from "../stores/exportDefaults"
import DumpExport from "../components/DumpExport.vue"

const store = useDatabaseStore()
const exportDefaults = useExportDefaultsStore()

const filter = ref("")
const dbFilter = ref("")
const dbSort = ref("name-asc")
const exporting = ref(false)
const copying = ref(false)

const loading = ref(false)
const loadingDatabases = ref(false)
const importing = ref(false)
const importPending = ref(false)
const importCancelled = ref(false)
const importPath = ref("")
const importFileName = ref("")

const tablesLocked = computed(() =>
  exporting.value || importing.value || copying.value
)
const databaseHasTables = computed(() => (store.tables || []).length > 0)
const skeletonCount = 6
const tableSkeletonCount = 8

const sortKey = ref("name")
const sortAsc = ref(true)

const filteredDatabases = computed(() => {
  const dbs = store.databases || []
  const term = dbFilter.value.trim().toLowerCase()

  const filtered = !term
    ? [...dbs]
    : dbs.filter(db =>
        String(db.name || db).toLowerCase().includes(term)
      )

  return filtered.sort((a, b) => {
    const nameA = String(a.name || a)
    const nameB = String(b.name || b)
    const sizeA = Number(a.size) || 0
    const sizeB = Number(b.size) || 0

    switch (dbSort.value) {
      case "name-desc":
        return nameB.localeCompare(nameA, undefined, { sensitivity: "base" })
      case "size-desc":
        return sizeB - sizeA || nameA.localeCompare(nameB, undefined, { sensitivity: "base" })
      case "size-asc":
        return sizeA - sizeB || nameA.localeCompare(nameB, undefined, { sensitivity: "base" })
      case "name-asc":
      default:
        return nameA.localeCompare(nameB, undefined, { sensitivity: "base" })
    }
  })
})

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

  loadingDatabases.value = true

  try{

    const dbs = await invoke("databases",{
      conn: store.connection.url || store.connection
    })

    store.databases = dbs

  }catch(e){

    console.error(e)

  }

  loadingDatabases.value = false

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

    store.tables = tables.map(t => {
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

function clearImportSelection(){
  importPending.value = false
  importPath.value = ""
  importFileName.value = ""
}

async function importDump(){

  if (!store.connection || !store.selectedDatabase || tablesLocked.value)
    return

  const path = await open({
    multiple: false,
    directory: false,
    filters: [{ name: "SQL", extensions: ["sql"] }],
    title: "Selecionar arquivo de dump"
  })

  if (!path)
    return

  const resolved = Array.isArray(path) ? path[0] : path
  if (!resolved)
    return

  importPath.value = resolved
  importFileName.value = String(resolved).split(/[\\/]/).pop() || ""
  importPending.value = true
  importCancelled.value = false

}

async function startImport(){

  if (!store.connection || !store.selectedDatabase || !importPath.value || importing.value)
    return

  importing.value = true
  importPending.value = false
  importCancelled.value = false
  const startedAt = Date.now()

  try{

    await invoke("import_dump", {
      conn: store.connection.url || store.connection,
      database: store.selectedDatabase,
      path: importPath.value
    })

    if (!importCancelled.value) {
      const sec = Math.max(0, Math.floor((Date.now() - startedAt) / 1000))
      const h = Math.floor(sec / 3600)
      const m = Math.floor((sec % 3600) / 60)
      const s = sec % 60
      const elapsed = h > 0
        ? `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`
        : `${m}:${String(s).padStart(2, "0")}`
      alert(`Dump importado com sucesso.\n\nTempo: ${elapsed}`)
      loadTables()
    }

  }catch(e){

    console.error(e)
    alert("Erro ao importar dump.\n\nDetalhes: " + e)

  }

  importing.value = false
  clearImportSelection()

}

function cancelImport(){

  importCancelled.value = true
  importing.value = false
  clearImportSelection()

}

function onExportingChange(value){
  exporting.value = value
}

function onCopyingChange(value){
  copying.value = value
}

function onCopyDone(payload){
  const sameHost =
    String(payload?.targetHost || "") === String(store.connection?.host || "") &&
    Number(payload?.targetPort || 3306) === Number(store.connection?.port || 3306)

  if (sameHost && payload?.targetDatabase === store.selectedDatabase)
    loadTables()
}

function toggleSort(key){

  if (tablesLocked.value)
    return

  if (sortKey.value === key) {

    sortAsc.value = !sortAsc.value

  } else {

    sortKey.value = key
    sortAsc.value = true

  }

}

function toggleAll(field){

  if (tablesLocked.value)
    return

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
    dbFilter.value = ""
    dbSort.value = "name-asc"
    filter.value = ""
    exporting.value = false
    importing.value = false
    copying.value = false
    clearImportSelection()
    loadDatabases()
  }
)

watch(
  () => store.selectedDatabase,
  () => {
    exporting.value = false
    importing.value = false
    copying.value = false
    clearImportSelection()
  }
)

</script>

<template>

  <div
    class="database-page"
    :style="{ '--accent': store.connection?.color || '#3a7afe' }"
  >
    <div
      v-if="store.selectedDatabase"
      class="database-controls-row"
    >
      <label class="database-tables-search-wrap">
        <i
          class="ri-search-line database-tables-search-icon"
          aria-hidden="true"
        ></i>
        <input
          class="database-tables-search"
          v-model="filter"
          placeholder="Buscar tabelas..."
          :disabled="tablesLocked"
        >
        <button
          v-if="filter"
          type="button"
          class="database-tables-search-clear"
          title="Limpar busca"
          :disabled="tablesLocked"
          @click="filter = ''"
        >
          <i class="ri-close-line"></i>
        </button>
      </label>

      <button
        type="button"
        class="database-tables-btn"
        title="Atualizar lista de tabelas"
        :disabled="!store.connection || !store.selectedDatabase || loading || tablesLocked"
        @click="loadTables"
      >
        <i class="ri-refresh-line"></i>
        <span>Atualizar tabelas</span>
      </button>

      <button
        type="button"
        class="database-tables-btn database-tables-btn--primary"
        title="Selecionar arquivo .sql para importar"
        :disabled="!store.connection || !store.selectedDatabase || loading || tablesLocked || importPending"
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
        <div
          class="database-dbs-header"
          :style="{ '--accent': store.connection?.color || '#3a7afe' }"
        >
          <div class="database-dbs-title-group">
            <h2 class="database-dbs-title">
              {{ store.connection?.name || 'Conexão sem nome' }}
            </h2>
            <p class="database-dbs-subtitle">
              <template v-if="loadingDatabases">
                Carregando databases...
              </template>
              <template v-else>
                {{ filteredDatabases.length }}
                <template v-if="dbFilter.trim()">
                  de {{ store.databases.length || 0 }}
                </template>
                databases disponíveis
              </template>
            </p>
          </div>

          <div class="database-dbs-controls">
            <label class="database-dbs-search-wrap">
              <i
                class="ri-search-line database-dbs-search-icon"
                aria-hidden="true"
              ></i>
              <input
                class="database-dbs-search"
                v-model="dbFilter"
                placeholder="Buscar databases..."
                :disabled="loadingDatabases"
              >
              <button
                v-if="dbFilter"
                type="button"
                class="database-dbs-search-clear"
                title="Limpar busca"
                @click="dbFilter = ''"
              >
                <i class="ri-close-line"></i>
              </button>
            </label>

            <span class="database-dbs-controls-sep" aria-hidden="true"></span>

            <label
              class="database-dbs-sort-wrap"
              title="Ordenar databases"
            >
              <i
                class="ri-arrow-up-down-line database-dbs-sort-icon"
                aria-hidden="true"
              ></i>
              <select
                v-model="dbSort"
                class="database-dbs-sort"
                :disabled="loadingDatabases"
              >
                <option value="name-asc">A → Z</option>
                <option value="name-desc">Z → A</option>
                <option value="size-desc">Maior → Menor</option>
                <option value="size-asc">Menor → Maior</option>
              </select>
              <i
                class="ri-arrow-down-s-line database-dbs-sort-caret"
                aria-hidden="true"
              ></i>
            </label>
          </div>
        </div>

        <div
          v-if="loadingDatabases"
          class="database-dbs-grid"
        >
          <div
            v-for="n in skeletonCount"
            :key="'sk-' + n"
            class="database-card database-card--skeleton"
            aria-hidden="true"
          >
            <div class="database-card-badge">
              <span class="skeleton-pill"></span>
            </div>
            <div class="database-card-body">
              <span class="skeleton-line skeleton-line--title"></span>
              <span class="skeleton-line skeleton-line--meta"></span>
            </div>
          </div>
        </div>

        <div
          v-else-if="!filteredDatabases.length"
          class="database-dbs-empty"
        >
          {{ dbFilter.trim() ? "Nenhuma database encontrada." : "Nenhuma database disponível." }}
        </div>

        <div v-else class="database-dbs-grid">
          <div
            v-for="db in filteredDatabases"
            :key="db.name"
            class="database-card"
            :style="{ '--accent': store.connection?.color || '#3a7afe' }"
            @click="store.selectedDatabase = db.name; loadTables()"
          >
            <div class="database-card-badge">
              <span class="database-status-badge">
                Database
              </span>
            </div>
            <div class="database-card-body">
              <div class="database-card-name">
                {{ db.name }}
              </div>
              <div class="database-card-size">
                {{ formatSize(db.size || 0) }}
              </div>
            </div>
          </div>
        </div>
      </div>

      <div
        v-else
        class="database-table-layout"
      >
        <div class="database-table-area">
          <div
            v-if="loading"
            class="table-container table-container--skeleton"
          >
            <table>
              <thead>
                <tr>
                  <th>Tabela</th>
                  <th>Tamanho</th>
                  <th class="col-action">S</th>
                  <th class="col-action">C</th>
                  <th class="col-action">D</th>
                </tr>
              </thead>
              <tbody>
                <tr
                  v-for="n in tableSkeletonCount"
                  :key="'table-sk-' + n"
                  class="table-skeleton-row"
                >
                  <td>
                    <span
                      class="skeleton-line skeleton-line--table-name"
                      :style="{ width: (48 + (n % 4) * 12) + '%' }"
                    ></span>
                  </td>
                  <td>
                    <span class="skeleton-line skeleton-line--table-size"></span>
                  </td>
                  <td class="col-action">
                    <span class="skeleton-check"></span>
                  </td>
                  <td class="col-action">
                    <span class="skeleton-check"></span>
                  </td>
                  <td class="col-action">
                    <span class="skeleton-check"></span>
                  </td>
                </tr>
              </tbody>
            </table>
          </div>

          <template v-else>
            <div
              class="table-container"
              :class="{ 'table-container--locked': tablesLocked }"
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
                        :disabled="tablesLocked"
                        :checked="filteredTables.length && filteredTables.every(t => t.structure)"
                        @change="toggleAll('structure')"
                      >
                      S
                    </th>
                    <th class="col-action">
                      <input
                        type="checkbox"
                        title="Content"
                        :disabled="tablesLocked"
                        :checked="filteredTables.length && filteredTables.every(t => t.content)"
                        @change="toggleAll('content')"
                      >
                      C
                    </th>
                    <th class="col-action">
                      <input
                        type="checkbox"
                        title="Drop"
                        :disabled="tablesLocked"
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
                        :disabled="tablesLocked"
                      >
                    </td>
                    <td class="col-action">
                      <input
                        type="checkbox"
                        v-model="t.content"
                        :disabled="tablesLocked"
                      >
                    </td>
                    <td class="col-action">
                      <input
                        type="checkbox"
                        v-model="t.drop"
                        :disabled="tablesLocked"
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
          </template>
        </div>

        <DumpExport
          :tables="store.tables"
          :importing="importing"
          :import-pending="importPending"
          :import-file-name="importFileName"
          :database-has-tables="databaseHasTables"
          @update:exporting="onExportingChange"
          @update:copying="onCopyingChange"
          @copy-done="onCopyDone"
          @start-import="startImport"
          @cancel-import="cancelImport"
        />
      </div>
    </div>
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
  grid-template-columns: minmax(0, 1fr) auto auto;
  gap: 10px;
  margin-bottom: 16px;
  align-items: center;
}

.database-tables-search-wrap {
  position: relative;
  display: flex;
  align-items: center;
  min-width: 0;
  height: 36px;
  border-radius: 999px;
  border: 1px solid color-mix(in srgb, var(--accent, #3a7afe) 45%, #27272a 55%);
  background:
    radial-gradient(
      circle at left center,
      color-mix(in srgb, var(--accent, #3a7afe) 16%, transparent),
      transparent 72%
    ),
    rgba(2, 6, 23, 0.95);
  box-shadow:
    inset 0 0 0 1px color-mix(in srgb, var(--accent, #3a7afe) 8%, transparent),
    0 10px 28px rgba(0, 0, 0, 0.35);
  overflow: hidden;
  transition: border-color 0.15s ease, box-shadow 0.15s ease;
}

.database-tables-search-wrap:focus-within {
  border-color: color-mix(in srgb, var(--accent, #3a7afe) 85%, #27272a 15%);
  box-shadow:
    0 0 0 1px color-mix(in srgb, var(--accent, #3a7afe) 50%, transparent),
    0 12px 32px rgba(0, 0, 0, 0.4);
}

.database-tables-search-icon {
  position: absolute;
  left: 14px;
  top: 50%;
  transform: translateY(-50%);
  color: color-mix(in srgb, var(--accent, #3a7afe) 70%, #9ca3af 30%);
  font-size: 15px;
  pointer-events: none;
}

.database-tables-search {
  width: 100%;
  height: 100%;
  border: none;
  outline: none;
  background: transparent;
  color: #e5e7eb;
  font-size: 13px;
  line-height: 1;
  padding: 0 40px 0 38px;
  border-radius: 0;
}

.database-tables-search::placeholder {
  color: #6b7280;
}

.database-tables-search:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.database-tables-search-clear {
  position: absolute;
  right: 8px;
  top: 50%;
  transform: translateY(-50%);
  width: 24px;
  height: 24px;
  border: none;
  border-radius: 999px;
  background: rgba(39, 39, 42, 0.9);
  color: #9ca3af;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  padding: 0;
  font-size: 14px;
}

.database-tables-search-clear:hover:not(:disabled) {
  color: #e5e7eb;
  background: color-mix(in srgb, var(--accent, #3a7afe) 25%, #27272a 75%);
}

.database-tables-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  height: 36px;
  padding: 0 14px;
  border-radius: 999px;
  border: 1px solid color-mix(in srgb, var(--accent, #3a7afe) 45%, #27272a 55%);
  background:
    radial-gradient(
      circle at left center,
      color-mix(in srgb, var(--accent, #3a7afe) 12%, transparent),
      transparent 70%
    ),
    rgba(2, 6, 23, 0.95);
  color: #e5e7eb;
  font-size: 13px;
  white-space: nowrap;
  cursor: pointer;
  box-shadow:
    inset 0 0 0 1px color-mix(in srgb, var(--accent, #3a7afe) 8%, transparent),
    0 10px 28px rgba(0, 0, 0, 0.25);
  transition: border-color 0.15s ease, box-shadow 0.15s ease, background 0.15s ease;
}

.database-tables-btn:hover:not(:disabled) {
  border-color: color-mix(in srgb, var(--accent, #3a7afe) 85%, #27272a 15%);
  box-shadow:
    0 0 0 1px color-mix(in srgb, var(--accent, #3a7afe) 40%, transparent),
    0 12px 32px rgba(0, 0, 0, 0.35);
}

.database-tables-btn--primary {
  border-color: color-mix(in srgb, var(--accent, #3a7afe) 70%, #27272a 30%);
  background:
    radial-gradient(
      circle at left center,
      color-mix(in srgb, var(--accent, #3a7afe) 28%, transparent),
      transparent 72%
    ),
    rgba(2, 6, 23, 0.95);
  color: color-mix(in srgb, var(--accent, #3a7afe) 35%, #f9fafb 65%);
}

.database-tables-btn:disabled {
  opacity: 0.45;
  cursor: not-allowed;
  box-shadow: none;
}

.database-table-layout {
  flex: 1;
  min-height: 0;
  display: flex;
  gap: 16px;
  align-items: stretch;
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
  padding-bottom: 20px;
}

.database-dbs-header {
  display: grid;
  grid-template-columns: minmax(160px, 1fr) auto minmax(160px, 1fr);
  align-items: center;
  gap: 16px;
  margin-bottom: 16px;
}

.database-dbs-title-group {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
}

.database-dbs-controls {
  grid-column: 2;
  display: flex;
  align-items: stretch;
  width: min(520px, 70vw);
  height: 36px;
  border-radius: 999px;
  border: 1px solid color-mix(in srgb, var(--accent, #3a7afe) 45%, #27272a 55%);
  background:
    radial-gradient(
      circle at left center,
      color-mix(in srgb, var(--accent, #3a7afe) 16%, transparent),
      transparent 72%
    ),
    rgba(2, 6, 23, 0.95);
  box-shadow:
    inset 0 0 0 1px color-mix(in srgb, var(--accent, #3a7afe) 8%, transparent),
    0 10px 28px rgba(0, 0, 0, 0.35);
  overflow: hidden;
  transition: border-color 0.15s ease, box-shadow 0.15s ease;
}

.database-dbs-controls:focus-within {
  border-color: color-mix(in srgb, var(--accent, #3a7afe) 85%, #27272a 15%);
  box-shadow:
    0 0 0 1px color-mix(in srgb, var(--accent, #3a7afe) 50%, transparent),
    0 12px 32px rgba(0, 0, 0, 0.4);
}

.database-dbs-controls-sep {
  width: 1px;
  align-self: stretch;
  background: color-mix(in srgb, var(--accent, #3a7afe) 35%, #27272a 65%);
  flex-shrink: 0;
}

.database-dbs-search-wrap {
  position: relative;
  display: flex;
  align-items: center;
  flex: 1;
  min-width: 0;
  height: 100%;
  border: none;
  border-radius: 0;
  background: transparent;
  box-shadow: none;
}

.database-dbs-search-icon {
  position: absolute;
  left: 14px;
  top: 50%;
  transform: translateY(-50%);
  color: color-mix(in srgb, var(--accent, #3a7afe) 70%, #9ca3af 30%);
  font-size: 15px;
  pointer-events: none;
}

.database-dbs-search {
  width: 100%;
  height: 100%;
  border: none;
  outline: none;
  background: transparent;
  color: #e5e7eb;
  font-size: 13px;
  line-height: 1;
  padding: 0 40px 0 38px;
  border-radius: 0;
}

.database-dbs-search::placeholder {
  color: #6b7280;
}

.database-dbs-search:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.database-dbs-search-clear {
  position: absolute;
  right: 8px;
  top: 50%;
  transform: translateY(-50%);
  width: 24px;
  height: 24px;
  border: none;
  border-radius: 999px;
  background: rgba(39, 39, 42, 0.9);
  color: #9ca3af;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  padding: 0;
  font-size: 14px;
}

.database-dbs-search-clear:hover {
  color: #e5e7eb;
  background: color-mix(in srgb, var(--accent, #3a7afe) 25%, #27272a 75%);
}

.database-dbs-sort-wrap {
  position: relative;
  display: inline-flex;
  align-items: center;
  flex-shrink: 0;
  min-width: 168px;
  height: 100%;
  border: none;
  border-radius: 0;
  background: transparent;
  box-shadow: none;
}

.database-dbs-sort-icon,
.database-dbs-sort-caret {
  position: absolute;
  top: 50%;
  transform: translateY(-50%);
  pointer-events: none;
  color: color-mix(in srgb, var(--accent, #3a7afe) 70%, #9ca3af 30%);
  font-size: 14px;
  line-height: 1;
}

.database-dbs-sort-icon {
  left: 12px;
}

.database-dbs-sort-caret {
  right: 10px;
  font-size: 16px;
  color: #9ca3af;
}

.database-dbs-sort {
  appearance: none;
  -webkit-appearance: none;
  width: 100%;
  height: 100%;
  min-width: 168px;
  border: none;
  outline: none;
  background: transparent;
  color: #e5e7eb;
  font-size: 12px;
  font-weight: 500;
  letter-spacing: 0.02em;
  line-height: 1;
  padding: 0 32px 0 34px;
  cursor: pointer;
  border-radius: 0;
}

.database-dbs-sort:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.database-dbs-sort option {
  background: #020617;
  color: #e5e7eb;
}

@media (max-width: 900px) {
  .database-dbs-header {
    grid-template-columns: minmax(0, 1fr);
  }

  .database-dbs-controls {
    grid-column: 1;
    width: 100%;
  }
}

.database-dbs-empty {
  padding: 24px 4px;
  font-size: 13px;
  color: #9ca3af;
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
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: 20px;
  align-content: flex-start;
}

.database-card {
  position: relative;
  aspect-ratio: 3 / 4;
  border-radius: 20px;
  padding: 18px 16px 16px;
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

.database-card--skeleton {
  cursor: default;
  pointer-events: none;
  overflow: hidden;
  border-color: #27272a;
  background:
    linear-gradient(
      135deg,
      rgba(24, 24, 27, 0.96),
      rgba(15, 23, 42, 1)
    );
  box-shadow:
    0 18px 40px rgba(0, 0, 0, 0.75),
    0 0 0 1px rgba(39, 39, 42, 0.8);
}

.database-card--skeleton::before {
  display: none;
}

.database-card--skeleton:hover {
  transform: none;
  box-shadow:
    0 18px 40px rgba(0, 0, 0, 0.75),
    0 0 0 1px rgba(39, 39, 42, 0.8);
  border-color: #27272a;
}

.skeleton-pill,
.skeleton-line {
  display: block;
  border-radius: 999px;
  background: linear-gradient(
    90deg,
    #1f2937 0%,
    #374151 50%,
    #1f2937 100%
  );
  background-size: 200% 100%;
  animation: database-skeleton-shimmer 1.2s ease-in-out infinite;
}

.skeleton-pill {
  width: 72px;
  height: 18px;
}

.skeleton-line--title {
  width: 78%;
  height: 16px;
  margin-bottom: 10px;
}

.skeleton-line--meta {
  width: 42%;
  height: 12px;
}

@keyframes database-skeleton-shimmer {
  0% { background-position: 100% 0; }
  100% { background-position: -100% 0; }
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
    0 0 0 1px color-mix(in srgb, var(--accent, #3a7afe) 80%, transparent);
  border-color: color-mix(in srgb, var(--accent, #3a7afe) 90%, #020617 10%);
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
  border: 1px solid color-mix(in srgb, var(--accent, #3a7afe) 65%, #27272a 35%);
  color: color-mix(in srgb, var(--accent, #3a7afe) 55%, #e5e7eb 45%);
  background: color-mix(in srgb, var(--accent, #3a7afe) 20%, rgba(31, 41, 55, 0.9));
}

.database-card-body {
  display: flex;
  flex-direction: column;
  justify-content: flex-end;
  flex: 1;
  padding-top: 12px;
}

.database-card-name {
  font-size: 22px;
  font-weight: 600;
  color: #f9fafb;
  word-break: break-all;
}

.database-card-size {
  margin-top: 8px;
  font-size: 13px;
  color: #9ca3af;
}

.database-table-area {
  flex: 1;
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.table-container--locked {
  opacity: 0.55;
  filter: grayscale(0.35);
  pointer-events: none;
}

.table-container--locked input[type="checkbox"] {
  cursor: not-allowed;
  opacity: 0.7;
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

.table-container--skeleton {
  pointer-events: none;
}

.table-skeleton-row td {
  vertical-align: middle;
}

.skeleton-line--table-name {
  height: 12px;
  max-width: 280px;
}

.skeleton-line--table-size {
  width: 64px;
  height: 12px;
}

.skeleton-check {
  display: inline-block;
  width: 14px;
  height: 14px;
  border-radius: 4px;
  background: linear-gradient(
    90deg,
    #1f2937 0%,
    #374151 50%,
    #1f2937 100%
  );
  background-size: 200% 100%;
  animation: database-skeleton-shimmer 1.2s ease-in-out infinite;
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
  padding-bottom: 20px;
  border-radius:12px;
  border: 1px solid color-mix(in srgb, var(--accent, #3a7afe) 28%, #1f2937 72%);
  background: rgba(2, 6, 23, 0.72);
}

.table-container :deep(table),
.table-container table {
  background: transparent;
}

.table-container thead {
  position: sticky;
  top: 0;
  z-index: 1;
  background: rgba(15, 23, 42, 0.96);
}

.table-container th {
  color: #9ca3af;
  border-bottom: 1px solid color-mix(in srgb, var(--accent, #3a7afe) 22%, #1f2937 78%);
}

.table-container td {
  border-bottom: 1px solid #111827;
}

.table-container tr:hover td {
  background: color-mix(in srgb, var(--accent, #3a7afe) 8%, transparent);
}

.table-container input[type="checkbox"] {
  border-color: color-mix(in srgb, var(--accent, #3a7afe) 45%, #3f3f46 55%);
  background: #020617;
}

.table-container input[type="checkbox"]:checked {
  border-color: color-mix(in srgb, var(--accent, #3a7afe) 85%, #27272a 15%);
  background: color-mix(in srgb, var(--accent, #3a7afe) 75%, #020617 25%);
}

.table-container input[type="checkbox"]:checked::after {
  background: #f9fafb;
}

.col-action{
  width:60px;
  text-align:center;
  white-space:nowrap;
}

</style>