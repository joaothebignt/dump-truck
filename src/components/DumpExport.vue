<script setup>

import { ref, reactive, computed } from "vue"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { useDatabaseStore } from "../stores/database"

const props = defineProps({
  tables: Array
})

const store = useDatabaseStore()

const exporting = ref(false)
const progress = ref(0)
const resultPath = ref("")
const tableStatuses = reactive({})

const hasSelectedTables = computed(() =>
  (props.tables || []).some(t => t.structure || t.content || t.drop)
)

const statusLabel = {
  pending: "Aguardando",
  dumping: "Exportando...",
  done: "Concluido",
  error: "Erro"
}

const statusIcon = {
  pending: "\u23F3",
  dumping: "\u26A1",
  done: "\u2705",
  error: "\u274C"
}

async function exportDump(){

  const selectedTables =
    (props.tables || []).filter(t => t.structure || t.content || t.drop)

  if (!selectedTables.length || !store.connection || !store.selectedDatabase)
    return

  exporting.value = true
  progress.value = 0
  resultPath.value = ""

  const tables = selectedTables.map(t => ({
    name: t.name,
    structure: t.structure,
    content: t.content,
    drop: t.drop
  }))

  for (const t of tables) {
    tableStatuses[t.name] = "pending"
  }

  const total = tables.length
  let unlisten = null

  try{

    unlisten = await listen("dump-progress", (event) => {
      const p = event.payload
      tableStatuses[p.table] = p.status

      if (p.completed && p.total) {
        progress.value = Math.round((p.completed / p.total) * 100)
      }
    })

    const result = await invoke("dump_parallel", {
      conn: store.connection,
      database: store.selectedDatabase,
      tables
    })

    resultPath.value = result.final_file

    if (!result.verified) {
      alert("Dump gerado, mas a verificacao de integridade falhou.\nArquivo: " + result.final_file)
    }

  }catch(e){

    console.error(e)
    alert("Erro ao gerar o dump.\n\nDetalhes: " + e)

  }finally{

    if (unlisten) unlisten()
    exporting.value = false

  }

}

function resetExport(){
  resultPath.value = ""
  Object.keys(tableStatuses).forEach(k => delete tableStatuses[k])
}

</script>

<template>

<div class="dump-export">

  <button
    @click="exportDump"
    :disabled="exporting || !hasSelectedTables"
  >
    {{ exporting ? "Exportando..." : "Export Dump" }}
  </button>

  <div
    v-if="exporting"
    class="export-status"
  >
    <div class="export-progress-bar">
      <div
        class="export-progress-bar-inner"
        :style="{ width: progress + '%' }"
      />
    </div>
    <div class="export-text">
      {{ progress }}%
    </div>
  </div>

  <div
    v-if="Object.keys(tableStatuses).length"
    class="table-status-list"
  >
    <div
      v-for="(status, name) in tableStatuses"
      :key="name"
      class="table-status-item"
      :class="'status-' + status"
    >
      <span class="table-status-icon">{{ statusIcon[status] }}</span>
      <span class="table-status-name">{{ name }}</span>
      <span class="table-status-label">{{ statusLabel[status] }}</span>
    </div>
  </div>

  <div
    v-if="resultPath && !exporting"
    class="export-result"
  >
    Dump salvo em: <span class="export-path">{{ resultPath }}</span>
    <button class="btn-reset" @click="resetExport">Limpar</button>
  </div>

</div>

</template>

<style scoped>

.dump-export{
  display:flex;
  flex-direction:column;
  gap:4px;
}

button[disabled]{
  opacity:0.6;
  cursor:not-allowed;
}

.export-status{
  display:flex;
  flex-direction:column;
  gap:2px;
  font-size:12px;
  color:#555;
}

.export-progress-bar{
  width:160px;
  height:4px;
  background:#e0e0e0;
  border-radius:999px;
  overflow:hidden;
}

.export-progress-bar-inner{
  height:100%;
  background:#3a7afe;
  border-radius:999px;
  transition:width 0.2s ease-out;
}

.export-text{
  white-space:nowrap;
}

.table-status-list{
  display:flex;
  flex-direction:column;
  gap:2px;
  margin-top:4px;
  max-height:200px;
  overflow-y:auto;
  font-size:12px;
}

.table-status-item{
  display:flex;
  align-items:center;
  gap:6px;
  padding:2px 6px;
  border-radius:4px;
  transition:background 0.2s, opacity 0.2s;
}

.table-status-icon{
  flex-shrink:0;
  width:16px;
  text-align:center;
}

.table-status-name{
  flex:1;
  overflow:hidden;
  text-overflow:ellipsis;
  white-space:nowrap;
  font-family:monospace;
}

.table-status-label{
  flex-shrink:0;
  font-size:11px;
  color:#888;
}

.status-pending{
  opacity:0.5;
}

.status-dumping{
  background:#e8f0fe;
  opacity:1;
}

.status-done{
  opacity:0.7;
}

.status-error{
  background:#fde8e8;
  opacity:1;
}

.status-error .table-status-label{
  color:#c53030;
}

.export-result{
  font-size:11px;
  color:#333;
  word-break:break-all;
  display:flex;
  align-items:center;
  gap:6px;
  flex-wrap:wrap;
}

.export-path{
  font-weight:600;
}

.btn-reset{
  font-size:10px;
  padding:1px 6px;
  cursor:pointer;
  border:1px solid #ccc;
  border-radius:4px;
  background:#f5f5f5;
}

.btn-reset:hover{
  background:#e0e0e0;
}

</style>
