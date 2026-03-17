<script setup>

import { ref, computed } from "vue"
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
const showFloatingToolbar = ref(false)

const hasSelectedTables = computed(() =>
  (props.tables || []).some(t => t.structure || t.content || t.drop)
)

async function exportDump(){

  const selectedTables =
    (props.tables || []).filter(t => t.structure || t.content || t.drop)

  if (!selectedTables.length || !store.connection || !store.selectedDatabase)
    return

  exporting.value = true
  progress.value = 0
  currentTable.value = ""
  cancelled.value = false
  showFloatingToolbar.value = true

  try{

    const total = selectedTables.length
    const parts = []

    for (let i = 0; i < total; i++) {

      if (cancelled.value)
        break

      const table = selectedTables[i]
      currentTable.value = table.name

      const sql = await invoke("dump_single_table",{
        conn: store.connection,
        database: store.selectedDatabase,
        table: table.name,
        structure: table.structure,
        content: table.content,
        drop: table.drop
      })

      parts.push(sql)
      progress.value = Math.round(((i + 1) / total) * 100)

    }

    if (cancelled.value || !parts.length)
      return

    const finalSql = parts.join("\n\n")

    const now = new Date()
    const date = now.toISOString().slice(0, 10).replace(/-/g, "")
    const time = now.toTimeString().slice(0, 8).replace(/:/g, "")
    const suggestedName = `${store.selectedDatabase}_${date}${time}.sql`

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
    showFloatingToolbar.value = false

  }

}

function cancelExport(){

  cancelled.value = true

}

</script>

<template>

<div class="dump-export">
  <button
    type="button"
    class="btn btn--ghost"
    @click="exportDump"
    :disabled="exporting || !hasSelectedTables"
  >
    <i class="ri-upload-2-line"></i>
    <span>{{ exporting ? "Exportando..." : "Exportar" }}</span>
  </button>

  <div
    v-if="showFloatingToolbar"
    class="dump-floating-toolbar"
  >
    <div class="dump-floating-main">
      <div class="dump-floating-info">
        <span class="dump-floating-label">
          Exportando:
        </span>
        <span class="dump-floating-table">
          {{ currentTable || 'Preparando...' }}
        </span>
      </div>

      <div class="dump-floating-progress">
        <div class="dump-floating-progress-bar">
          <div
            class="dump-floating-progress-bar-inner"
            :style="{ width: progress + '%' }"
          />
        </div>
        <span class="dump-floating-percent">
          {{ progress }}%
        </span>
      </div>
    </div>

    <button
      type="button"
      class="dump-floating-cancel"
      title="Cancelar exportação"
      @click="cancelExport"
    >
      <i class="ri-close-line"></i>
    </button>
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

.dump-floating-toolbar{
  position: fixed;
  left: 50%;
  bottom: 16px;
  transform: translateX(-50%);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 24px;
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

.dump-floating-main{
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.dump-floating-info{
  display: flex;
  flex-direction: row;
  gap: 8px;
  align-items: baseline;
}

.dump-floating-label{
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.12em;
  color: #9ca3af;
}

.dump-floating-table{
  font-size: 13px;
  font-weight: 500;
  max-width: 220px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.dump-floating-progress{
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
}

.dump-floating-progress-bar{
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

.dump-floating-progress-bar-inner{
  height:100%;
  background:#3a7afe;
  border-radius:999px;
  transition:width 0.2s ease-out;
}

.dump-floating-percent{
  font-size: 12px;
  color: #e5e7eb;
}

.dump-floating-cancel{
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