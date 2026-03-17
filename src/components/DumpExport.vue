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
const resultPath = ref("")
const tableStatuses = reactive({})
const completedAt = ref(null)
const hasError = ref(false)

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

const errorTables = computed(() => {
  return Object.entries(tableStatuses)
    .filter(([_, data]) => data.status === "error")
    .map(([name, data]) => ({ name, error: data.error }))
})

const successCount = computed(() => {
  return Object.values(tableStatuses).filter(d => d.status === "done").length
})

const totalCount = computed(() => {
  return Object.keys(tableStatuses).length
})

async function exportDump(){

  const selectedTables =
    (props.tables || []).filter(t => t.structure || t.content || t.drop)

  if (!selectedTables.length || !store.connection || !store.selectedDatabase)
    return

  exporting.value = true
  resultPath.value = ""
  hasError.value = false
  completedAt.value = null

  const tables = selectedTables.map(t => ({
    name: t.name,
    structure: t.structure,
    content: t.content,
    drop: t.drop,
    size: t.size || 0
  }))

  for (const t of tables) {
    tableStatuses[t.name] = { status: "pending", progress: 0 }
  }

  let unlisten = null

  try{

    unlisten = await listen("dump-progress", (event) => {
      const p = event.payload
      const progress = (p.total > 0) ? Math.round((p.completed / p.total) * 100) : 0

      tableStatuses[p.table] = {
        status: p.status,
        progress: progress,
        error: p.error || null
      }
    })

    const result = await invoke("dump_parallel", {
      conn: store.connection,
      database: store.selectedDatabase,
      tables
    })

    resultPath.value = result.final_file
    completedAt.value = new Date()

    if (!result.verified) {
      alert("Dump gerado, mas a verificacao de integridade falhou.\nArquivo: " + result.final_file)
    }

  }catch(e){

    console.error(e)
    hasError.value = true
    alert("Erro ao gerar o dump.\n\nDetalhes: " + e)

  }finally{

    if (unlisten) unlisten()
    exporting.value = false

  }

}

function resetExport(){
  resultPath.value = ""
  completedAt.value = null
  hasError.value = false
  Object.keys(tableStatuses).forEach(k => delete tableStatuses[k])
}

function showError(error){
  alert("Erro ao exportar:\n\n" + error)
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
    v-if="Object.keys(tableStatuses).length"
    class="table-status-list"
  >
    <div
      v-for="(data, name) in tableStatuses"
      :key="name"
      class="table-status-item"
      :class="'status-' + data.status"
      :title="data.error || ''"
    >
      <span class="table-status-icon">{{ statusIcon[data.status] }}</span>
      <span class="table-status-name">{{ name }}</span>
      <span class="table-status-label">
        <template v-if="data.status === 'dumping' && data.progress > 0">
          {{ data.progress }}%
        </template>
        <template v-else-if="data.status === 'error' && data.error">
          <span class="error-text" @click="showError(data.error)">Erro</span>
        </template>
        <template v-else>
          {{ statusLabel[data.status] }}
        </template>
      </span>
    </div>
  </div>

  <!-- Secao de Erros -->
  <div v-if="errorTables.length > 0" class="error-section">
    <div class="error-header">
      <span class="error-icon">⚠️</span>
      <span class="error-title">{{ errorTables.length }} tabela(s) com erro</span>
    </div>
    <div class="error-list">
      <div v-for="item in errorTables" :key="item.name" class="error-item">
        <span class="error-table-name">{{ item.name }}</span>
        <button class="btn-error-detail" @click="showError(item.error)">Ver detalhes</button>
      </div>
    </div>
  </div>

  <!-- Feedback de Conclusao -->
  <div
    v-if="completedAt && !exporting"
    class="completion-section"
    :class="{ 'completion-success': !hasError && errorTables.length === 0, 'completion-error': hasError || errorTables.length > 0 }"
  >
    <div class="completion-header">
      <span class="completion-icon">{{ errorTables.length > 0 ? '⚠️' : '✅' }}</span>
      <span class="completion-title">
        {{ errorTables.length > 0 ? 'Dump concluido com erros' : 'Dump concluido com sucesso!' }}
      </span>
    </div>
    <div class="completion-stats">
      <span class="stat-success">{{ successCount }}/{{ totalCount }} tabelas</span>
      <span v-if="errorTables.length > 0" class="stat-error">{{ errorTables.length }} erro(s)</span>
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
  min-width:50px;
  text-align:right;
}

.status-pending{
  opacity:0.5;
}

.status-dumping{
  background:#e8f0fe;
  opacity:1;
}

.status-dumping .table-status-label{
  color:#3a7afe;
  font-weight:600;
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

.error-text{
  cursor:pointer;
  text-decoration:underline;
}

.error-text:hover{
  color:#9b2c2c;
}

/* Secao de Erros */
.error-section{
  margin-top:8px;
  padding:8px;
  background:#fff5f5;
  border:1px solid #fc8181;
  border-radius:6px;
}

.error-header{
  display:flex;
  align-items:center;
  gap:6px;
  font-weight:600;
  color:#c53030;
  margin-bottom:6px;
}

.error-icon{
  font-size:14px;
}

.error-title{
  font-size:12px;
}

.error-list{
  display:flex;
  flex-direction:column;
  gap:4px;
}

.error-item{
  display:flex;
  align-items:center;
  justify-content:space-between;
  padding:4px 8px;
  background:#fff;
  border-radius:4px;
  font-size:11px;
}

.error-table-name{
  font-family:monospace;
  color:#742a2a;
}

.btn-error-detail{
  font-size:10px;
  padding:2px 6px;
  cursor:pointer;
  border:1px solid #fc8181;
  border-radius:4px;
  background:#fff;
  color:#c53030;
}

.btn-error-detail:hover{
  background:#fed7d7;
}

/* Secao de Conclusao */
.completion-section{
  margin-top:8px;
  padding:10px;
  border-radius:6px;
}

.completion-success{
  background:#f0fff4;
  border:1px solid #68d391;
}

.completion-error{
  background:#fffaf0;
  border:1px solid #f6ad55;
}

.completion-header{
  display:flex;
  align-items:center;
  gap:8px;
  margin-bottom:4px;
}

.completion-icon{
  font-size:16px;
}

.completion-title{
  font-weight:600;
  font-size:13px;
}

.completion-success .completion-title{
  color:#276749;
}

.completion-error .completion-title{
  color:#c05621;
}

.completion-stats{
  display:flex;
  gap:12px;
  font-size:11px;
}

.stat-success{
  color:#38a169;
}

.stat-error{
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