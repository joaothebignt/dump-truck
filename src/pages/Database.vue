<script setup>

import { ref, computed, onMounted } from "vue"
import { invoke } from "@tauri-apps/api/core"

import { useDatabaseStore } from "../stores/database"
import DumpExport from "../components/DumpExport.vue"

const store = useDatabaseStore()

const filter = ref("")

const loading = ref(false)

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

  const selected = store.tables.filter(t => t.selected)

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
      selected: true

    }))

  }catch(e){

    console.error(e)

  }

  loading.value = false

}

function toggleSort(key){

  if (sortKey.value === key) {

    sortAsc.value = !sortAsc.value

  } else {

    sortKey.value = key
    sortAsc.value = true

  }

}

function toggleAll(){

  const visible = filteredTables.value

  if (!visible.length)
    return

  const allVisibleSelected = visible.every(t => t.selected)

  // só altera as tabelas visíveis (respeitando o filtro),
  // mantendo o estado das demais
  visible.forEach(t => {
    t.selected = !allVisibleSelected
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

    <input
      v-model="filter"
      placeholder="Filtrar tabelas..."
    >

    <DumpExport :tables="filteredTables" />

    <span
      v-if="selectedSummary.count"
      class="summary summary--inline"
    >
      {{ selectedSummary.count }} tabelas:
      {{ formatSize(selectedSummary.bytes) }}
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

  <table v-if="!loading">

    <thead>

      <tr>
        <th>
          <input
            type="checkbox"
            :checked="filteredTables.length && filteredTables.every(t => t.selected)"
            @change="toggleAll"
          >
        </th>
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
      </tr>

    </thead>

    <tbody>

      <tr
        v-for="t in filteredTables"
        :key="t.name"
      >

        <td>
          <input
            type="checkbox"
            v-model="t.selected"
          >
        </td>

        <td>{{t.name}}</td>

        <td>{{formatSize(t.size)}}</td>

      </tr>

    </tbody>

  </table>

  <div
    v-if="selectedSummary.count"
    class="summary"
  >
    {{ selectedSummary.count }} tabelas:
    {{ (selectedSummary.bytes/1024/1024).toFixed(2) }} MB
  </div>

</div>

</template>

<style scoped>

.database-page{
  display:flex;
  flex-direction:column;
}

.toolbar{
  display:flex;
  gap:10px;
  margin-bottom:15px;
}

select,input{
  padding:6px;
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

@keyframes progress-pulse{
  0%{ transform:translateX(-100%); }
  50%{ transform:translateX(0%); }
  100%{ transform:translateX(100%); }
}

</style>