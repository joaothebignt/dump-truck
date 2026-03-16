<script setup>

import { ref } from "vue"
import { invoke } from "@tauri-apps/api/core"
import { save } from "@tauri-apps/plugin-dialog"
import { writeTextFile } from "@tauri-apps/plugin-fs"
import { useDatabaseStore } from "../stores/database"

const props = defineProps({
  tables: Array
})

const store = useDatabaseStore()

const exporting = ref(false)
const progress = ref(0)
const currentTable = ref("")
const cancelled = ref(false)

async function exportDump(){

  const selectedTables =
    (props.tables || []).filter(t => t.selected).map(t => t.name)

  if (!selectedTables.length || !store.connection || !store.selectedDatabase)
    return

  exporting.value = true
  progress.value = 0
  currentTable.value = ""
  cancelled.value = false

  try{

    const total = selectedTables.length
    const parts = []

    for (let i = 0; i < total; i++) {

      if (cancelled.value)
        break

      const table = selectedTables[i]
      currentTable.value = table

      const sql = await invoke("dump_single_table",{
        conn: store.connection,
        database: store.selectedDatabase,
        table
      })

      parts.push(sql)
      progress.value = Math.round(((i + 1) / total) * 100)

    }

    if (cancelled.value || !parts.length)
      return

    const finalSql = parts.join("\n\n")

    const suggestedName = `${store.selectedDatabase}.sql`

    const filePath = await save({
      defaultPath: suggestedName,
      filters: [
        { name: "SQL Dump", extensions: ["sql"] }
      ]
    })

    if (!filePath) {
      // usuário cancelou
      return
    }

    await writeTextFile(filePath, finalSql)

  }catch(e){

    console.error(e)
    alert("Erro ao gerar o dump.\n\nDetalhes: " + e)

  }finally{

    exporting.value = false
    currentTable.value = ""

  }

}

function cancelExport(){

  cancelled.value = true

}

</script>

<template>

<div class="dump-export">

  <button
    @click="exportDump"
    :disabled="exporting"
  >
    {{ exporting ? "Exportando..." : "Export Dump" }}
  </button>

  <button
    v-if="exporting"
    class="cancel-btn"
    @click="cancelExport"
  >
    Cancelar
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
      {{ progress }}% - lendo tabela: {{ currentTable }}
    </div>
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

.cancel-btn{
  align-self:flex-start;
  font-size:11px;
  padding:2px 6px;
  border-radius:4px;
  border:1px solid #ccc;
  background:#f8f8f8;
  cursor:pointer;
}

</style>