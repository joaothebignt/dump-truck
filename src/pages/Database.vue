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
      conn: store.connection
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

      conn: store.connection,

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

  try{

    await invoke("import_dump", {
      conn: store.connection,
      database: store.selectedDatabase,
      path
    })

    alert("Dump importado com sucesso.")
    loadTables()

  }catch(e){

    console.error(e)
    alert("Erro ao importar dump.\n\nDetalhes: " + e)

  }

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

  <div class="toolbar">

    <select
      v-model="store.selectedDatabase"
      @change="loadTables"
    >

      <option
        v-for="db in store.databases"
        :key="db"
        :value="db"
      >
        {{db}}
      </option>

    </select>

    <button
      type="button"
      class="btn-refresh"
      title="Atualizar lista de bancos"
      :disabled="!store.connection || loading"
      @click="loadDatabases"
    >
      Atualizar bancos
    </button>

    <button
      type="button"
      class="btn-refresh"
      title="Atualizar lista de tabelas"
      :disabled="!store.connection || !store.selectedDatabase || loading"
      @click="loadTables"
    >
      Atualizar tabelas
    </button>

    <button
      type="button"
      class="btn-import"
      title="Importar arquivo .sql no banco selecionado"
      :disabled="!store.connection || !store.selectedDatabase || loading || importing"
      @click="importDump"
    >
      {{ importing ? "Importando..." : "Importar dump" }}
    </button>

    <input
      v-model="filter"
      placeholder="Filtrar tabelas..."
    >

    <DumpExport :tables="filteredTables" />

    <span
      v-if="store.tables && store.tables.length"
      class="summary summary--inline"
    >
      <template v-if="selectedSummary.count">
        {{ selectedSummary.count }} tabelas:
        {{ formatSize(selectedSummary.bytes) }}
      </template>
      <template v-else>
        Nenhuma tabela selecionada
      </template>
    </span>

  </div>

  <div v-if="loading">
    <div class="loading">
      <div class="progress-bar">
        <div class="progress-bar-inner"></div>
      </div>
      <span>Carregando tabelas...</span>
    </div>
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

          <td>{{t.name}}</td>

          <td>{{formatSize(t.size)}}</td>

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

</template>

<style scoped>

.database-page{
  display:flex;
  flex-direction:column;
  height:100%;
  min-height:0;
}

.toolbar{
  display:flex;
  flex-wrap:wrap;
  align-items:center;
  gap:10px;
  margin-bottom:15px;
}

select,input{
  padding:6px;
}

.btn-refresh{
  padding:6px 10px;
  font-size:13px;
  border:1px solid #ccc;
  border-radius:4px;
  background:#fff;
  cursor:pointer;
}

.btn-refresh:hover:not(:disabled){
  background:#f0f0f0;
}

.btn-refresh:disabled{
  opacity:0.6;
  cursor:not-allowed;
}

.btn-import{
  padding:6px 10px;
  font-size:13px;
  border:1px solid #0a7;
  border-radius:4px;
  background:#e8f8f5;
  color:#057;
  cursor:pointer;
}

.btn-import:hover:not(:disabled){
  background:#d0f0eb;
}

.btn-import:disabled{
  opacity:0.6;
  cursor:not-allowed;
}

table{
  width:100%;
  border-collapse:collapse;
  background:white;
}

th,td{
  border:1px solid #ddd;
  padding:6px;
}

th{
  background:#f5f5f5;
  position:sticky;
  top:0;
  z-index:1;
  box-shadow:0 1px 0 #ddd;
}

.sortable{
  cursor:pointer;
}

.sortable:hover{
  text-decoration:underline;
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
  color:#444;
}

.summary--inline{
  margin-top:0;
  margin-left:auto;
}

.table-container{
  flex:1;
  min-height:0;
  overflow-y:auto;
  border-radius:4px;
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

</style>