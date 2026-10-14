<script setup>

import { ref, computed, watch, onMounted, onUnmounted } from "vue"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { open, save } from "@tauri-apps/plugin-dialog"
import { useConnectionsStore } from "../stores/connections"
import { useDatabaseStore } from "../stores/database"
import {
  useExportDefaultsStore,
  selectionFromExportDefaults
} from "../stores/exportDefaults"

const props = defineProps({
  tables: Array,
  importing: {
    type: Boolean,
    default: false
  },
  importPending: {
    type: Boolean,
    default: false
  },
  importFileName: {
    type: String,
    default: ""
  },
  databaseHasTables: {
    type: Boolean,
    default: false
  }
})

const emit = defineEmits([
  "update:exporting",
  "update:copying",
  "cancel-import",
  "start-import",
  "copy-done"
])

const store = useDatabaseStore()
const connections = useConnectionsStore()
const defaultsStore = useExportDefaultsStore()

const destinationMode = ref("file") // file | connection

const exporting = ref(false)
const copying = ref(false)
const copyPhase = ref("dump")
const progress = ref(0)
const currentTable = ref("")
const exportedCount = ref(0)
const totalTables = ref(0)
const cancelled = ref(false)

const elapsedMs = ref(0)
let timerStartedAt = 0
let timerInterval = null

const busy = computed(() =>
  exporting.value || copying.value || props.importing
)

const elapsedLabel = computed(() => formatElapsed(elapsedMs.value))

const importConfirmNonEmpty = ref(false)
const canStartImport = computed(() =>
  !busy.value && (!props.databaseHasTables || importConfirmNonEmpty.value)
)

const fileName = ref("")
const saveDir = ref("")
const draftRules = ref([])
const newRuleType = ref("suffix")
const newRuleValue = ref("")
const newRuleExcludeStructure = ref(true)
const newRuleExcludeContent = ref(true)
const newRuleExcludeDrop = ref(true)

const targetIndex = ref("")
const targetDatabase = ref("")
const targetDatabases = ref([])
const loadingTargets = ref(false)
const targetTableCount = ref(null)
const copyConfirmNonEmpty = ref(false)

const hasSelectedTables = computed(() =>
  (props.tables || []).some(t => t.structure || t.content || t.drop)
)

const exportTables = computed(() => {
  const list = (props.tables || [])
    .filter(t => t.structure || t.content || t.drop)

  return [...list].sort((a, b) =>
    String(a.name).localeCompare(String(b.name), undefined, { sensitivity: "base" })
  )
})

const estimatedBytes = computed(() =>
  exportTables.value.reduce((sum, t) => {
    if (t.content)
      return sum + (t.size || 0)
    return sum
  }, 0)
)

const defaultFileName = computed(() => {
  const db = store.selectedDatabase || "dump"
  const now = new Date()
  const date = now.toISOString().slice(0, 10).replace(/-/g, "")
  const time = now.toTimeString().slice(0, 8).replace(/:/g, "")
  return `${db}_${date}${time}.sql`
})

const panelTitle = computed(() => {
  if (props.importing || props.importPending)
    return "Importar dump"
  if (copying.value)
    return "Copiar database"
  if (exporting.value)
    return "Exportar dump"
  return destinationMode.value === "connection" ? "Copiar database" : "Exportar dump"
})

const targetConnection = computed(() => {
  const i = Number(targetIndex.value)
  if (Number.isNaN(i) || i < 0 || i >= connections.favorites.length)
    return null
  return connections.favorites[i]
})

const sameTargetAsSource = computed(() => {
  const t = targetConnection.value
  if (!t || !store.connection || !store.selectedDatabase || !targetDatabase.value)
    return false
  const sameHost =
    String(t.host || "") === String(store.connection.host || "") &&
    Number(t.port || 3306) === Number(store.connection.port || 3306)
  return sameHost && targetDatabase.value === store.selectedDatabase
})

const targetHasTables = computed(() =>
  targetTableCount.value !== null && targetTableCount.value > 0
)

const canStartCopy = computed(() =>
  hasSelectedTables.value &&
  !!targetConnection.value &&
  !!targetDatabase.value &&
  !busy.value &&
  !loadingTargets.value &&
  !sameTargetAsSource.value &&
  (!targetHasTables.value || copyConfirmNonEmpty.value)
)

const canStartAction = computed(() => {
  if (destinationMode.value === "connection")
    return canStartCopy.value
  return hasSelectedTables.value && !busy.value
})

function formatSize(bytes) {
  if (bytes < 1024)
    return bytes + " B"
  if (bytes < 1024 * 1024)
    return (bytes / 1024).toFixed(1) + " KB"
  if (bytes < 1024 * 1024 * 1024)
    return (bytes / 1024 / 1024).toFixed(2) + " MB"
  return (bytes / 1024 / 1024 / 1024).toFixed(2) + " GB"
}

function formatElapsed(ms) {
  const totalSec = Math.max(0, Math.floor(Number(ms) / 1000))
  const h = Math.floor(totalSec / 3600)
  const m = Math.floor((totalSec % 3600) / 60)
  const s = totalSec % 60
  if (h > 0)
    return `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`
  return `${m}:${String(s).padStart(2, "0")}`
}

function startTimer() {
  stopTimer(false)
  timerStartedAt = Date.now()
  elapsedMs.value = 0
  timerInterval = setInterval(() => {
    elapsedMs.value = Date.now() - timerStartedAt
  }, 200)
}

function stopTimer(keepValue = true) {
  if (timerInterval) {
    clearInterval(timerInterval)
    timerInterval = null
  }
  if (keepValue && timerStartedAt)
    elapsedMs.value = Date.now() - timerStartedAt
  if (!keepValue) {
    timerStartedAt = 0
    elapsedMs.value = 0
  }
}

function takeElapsedLabel() {
  if (timerStartedAt)
    elapsedMs.value = Date.now() - timerStartedAt
  return formatElapsed(elapsedMs.value)
}

function isCancelledError(e) {
  const msg = typeof e === "string" ? e : (e?.message || String(e || ""))
  return msg.includes("CANCELLED")
}

function ensureSqlExtension(name) {
  const trimmed = String(name || "").trim()
  if (!trimmed)
    return defaultFileName.value
  return trimmed.toLowerCase().endsWith(".sql") ? trimmed : `${trimmed}.sql`
}

function joinPath(dir, name) {
  const normalizedDir = String(dir || "").replace(/[\\/]+$/, "")
  const sep = normalizedDir.includes("\\") ? "\\" : "/"
  return `${normalizedDir}${sep}${name}`
}

function syncDraftFromStore() {
  draftRules.value = defaultsStore.rules.map(r => ({
    type: r.type,
    value: r.value,
    excludeStructure: r.excludeStructure !== false,
    excludeContent: r.excludeContent !== false,
    excludeDrop: r.excludeDrop !== false
  }))
  if (!fileName.value)
    fileName.value = defaultFileName.value
  if (!saveDir.value && defaultsStore.lastSaveDir)
    saveDir.value = defaultsStore.lastSaveDir
}

function ruleExcludeLabel(rule) {
  const parts = []
  if (rule.excludeStructure !== false) parts.push("S")
  if (rule.excludeContent !== false) parts.push("C")
  if (rule.excludeDrop !== false) parts.push("D")
  return parts.join("") || "—"
}

function addDraftRule() {
  const value = String(newRuleValue.value || "").trim()
  if (!value)
    return
  if (!newRuleExcludeStructure.value && !newRuleExcludeContent.value && !newRuleExcludeDrop.value)
    return

  draftRules.value.push({
    type: newRuleType.value,
    value,
    excludeStructure: newRuleExcludeStructure.value,
    excludeContent: newRuleExcludeContent.value,
    excludeDrop: newRuleExcludeDrop.value
  })
  newRuleValue.value = ""
}

function removeDraftRule(index) {
  draftRules.value.splice(index, 1)
}

function applyDraftRulesToSelection() {
  defaultsStore.setRules(draftRules.value)
  if (!props.tables)
    return
  props.tables.forEach(t => {
    const sel = selectionFromExportDefaults(t.name, draftRules.value)
    t.structure = sel.structure
    t.content = sel.content
    t.drop = sel.drop
  })
}

async function chooseSaveDir() {
  const dir = await open({
    directory: true,
    multiple: false,
    title: "Escolher pasta para salvar o dump"
  })

  if (!dir)
    return

  saveDir.value = typeof dir === "string" ? dir : String(dir)
  defaultsStore.setLastSaveDir(saveDir.value)
}

async function connectionUrl(connection) {
  return invoke("connect_db", {
    host: connection.host,
    port: Number(connection.port) || 3306,
    user: connection.username,
    pass: connection.password
  })
}

async function loadTargetDatabases({ preserveSelection = false } = {}) {
  const previous = preserveSelection ? targetDatabase.value : ""
  if (!preserveSelection) {
    targetDatabases.value = []
    targetDatabase.value = ""
    targetTableCount.value = null
    copyConfirmNonEmpty.value = false
  }

  const conn = targetConnection.value
  if (!conn)
    return

  loadingTargets.value = true
  try {
    const url = await connectionUrl(conn)
    const dbs = await invoke("databases", { conn: url })
    targetDatabases.value = dbs || []

    if (preserveSelection && previous) {
      const stillExists = targetDatabases.value.some(
        db => String(db.name || db) === previous
      )
      targetDatabase.value = stillExists ? previous : ""
      if (stillExists)
        await refreshTargetTableCount()
      else {
        targetTableCount.value = null
        copyConfirmNonEmpty.value = false
      }
    }
  } catch (e) {
    console.error(e)
    alert("Erro ao listar databases do destino.\n\nDetalhes: " + e)
  }
  loadingTargets.value = false
}

function refreshTargetDatabases() {
  if (!targetConnection.value || loadingTargets.value)
    return
  return loadTargetDatabases({ preserveSelection: true })
}

async function refreshTargetTableCount() {
  targetTableCount.value = null
  copyConfirmNonEmpty.value = false

  const conn = targetConnection.value
  if (!conn || !targetDatabase.value)
    return

  try {
    const url = await connectionUrl(conn)
    const tables = await invoke("tables_with_size", {
      conn: url,
      database: targetDatabase.value
    })
    targetTableCount.value = (tables || []).length
  } catch (e) {
    console.error(e)
    targetTableCount.value = null
  }
}

async function startAction() {
  if (destinationMode.value === "connection")
    return startCopy()
  return startExport()
}

async function startExport() {
  const selected = exportTables.value

  if (!selected.length || !store.connection || !store.selectedDatabase || exporting.value)
    return

  defaultsStore.setRules(draftRules.value)

  const resolvedName = ensureSqlExtension(fileName.value)
  fileName.value = resolvedName

  let filePath = ""

  if (saveDir.value.trim()) {
    filePath = joinPath(saveDir.value.trim(), resolvedName)
    defaultsStore.setLastSaveDir(saveDir.value.trim())
  } else {
    const picked = await save({
      defaultPath: resolvedName,
      filters: [{ name: "SQL Dump", extensions: ["sql"] }]
    })

    if (!picked)
      return

    filePath = picked

    const parts = filePath.split(/[\\/]/)
    if (parts.length > 1) {
      const dir = parts.slice(0, -1).join(filePath.includes("\\") ? "\\" : "/")
      saveDir.value = dir
      defaultsStore.setLastSaveDir(dir)
    }
  }

  await runExport(selected, filePath)
}

async function runExport(selectedTables, filePath) {
  exporting.value = true
  emit("update:exporting", true)
  progress.value = 0
  currentTable.value = ""
  cancelled.value = false
  startTimer()

  let stopListen = null
  let succeeded = false

  try {
    stopListen = await listen("dump-progress", (event) => {
      const payload = event.payload || {}
      if (payload.table)
        currentTable.value = payload.table

      const total = Number(payload.total) || 0
      const index = Number(payload.index) || 0
      if (total > 0) {
        totalTables.value = total
        exportedCount.value = Math.min(index, total)
        progress.value = Math.min(99, Math.round((index / total) * 100))
      }
    })

    await invoke("prepare_dump")

    const ordered = [...selectedTables]
      .filter(t => t.structure || t.content)
      .sort((a, b) =>
        String(a.name).localeCompare(String(b.name), undefined, { sensitivity: "base" })
      )

    if (!ordered.length || cancelled.value)
      return

    currentTable.value = "Preparando..."
    totalTables.value = ordered.length
    exportedCount.value = 0
    progress.value = 1

    await invoke("dump_export", {
      conn: store.connection.url || store.connection,
      database: store.selectedDatabase,
      tables: ordered.map(t => ({
        name: t.name,
        structure: !!t.structure,
        content: !!t.content,
        drop: !!t.drop
      })),
      outputPath: filePath
    })

    if (cancelled.value)
      return

    exportedCount.value = totalTables.value
    progress.value = 100
    succeeded = true
    const duration = takeElapsedLabel()
    alert(`Dump exportado com sucesso.\n\nTempo: ${duration}`)

  } catch (e) {
    if (cancelled.value || isCancelledError(e))
      return

    console.error(e)
    alert("Erro ao gerar o dump.\n\nDetalhes: " + e)

  } finally {
    if (typeof stopListen === "function")
      stopListen()

    stopTimer(!succeeded)
    exporting.value = false
    emit("update:exporting", false)
    currentTable.value = ""
    exportedCount.value = 0
    totalTables.value = 0
    progress.value = 0
    if (!succeeded)
      stopTimer(false)
  }
}

async function startCopy() {
  if (!canStartCopy.value || !store.connection || !store.selectedDatabase)
    return

  defaultsStore.setRules(draftRules.value)

  const sourceUrl = store.connection.url || store.connection
  let targetUrl

  try {
    targetUrl = await connectionUrl(targetConnection.value)
  } catch (e) {
    alert("Erro ao conectar no destino.\n\nDetalhes: " + e)
    return
  }

  const selected = exportTables.value
    .filter(t => t.structure || t.content)
    .map(t => ({
      name: t.name,
      structure: !!t.structure,
      content: !!t.content,
      drop: !!t.drop
    }))

  if (!selected.length)
    return

  copying.value = true
  emit("update:copying", true)
  copyPhase.value = "dump"
  progress.value = 1
  currentTable.value = "Preparando..."
  exportedCount.value = 0
  totalTables.value = selected.length
  cancelled.value = false
  startTimer()

  let stopDump = null
  let stopCopy = null
  let succeeded = false

  try {
    stopDump = await listen("dump-progress", (event) => {
      const payload = event.payload || {}
      if (payload.table)
        currentTable.value = payload.table
      const total = Number(payload.total) || 0
      const index = Number(payload.index) || 0
      if (total > 0) {
        totalTables.value = total
        exportedCount.value = Math.min(index, total)
        progress.value = Math.min(90, Math.round((index / total) * 90))
      }
    })

    stopCopy = await listen("copy-progress", (event) => {
      const payload = event.payload || {}
      if (payload.phase)
        copyPhase.value = payload.phase
      if (payload.table)
        currentTable.value = payload.table
      if (payload.phase === "import") {
        progress.value = 95
        currentTable.value = `Importando em ${targetDatabase.value}...`
      }
    })

    await invoke("copy_database", {
      sourceConn: sourceUrl,
      sourceDatabase: store.selectedDatabase,
      targetConn: targetUrl,
      targetDatabase: targetDatabase.value,
      tables: selected
    })

    if (cancelled.value)
      return

    progress.value = 100
    succeeded = true
    const duration = takeElapsedLabel()
    alert(
      `Database copiado com sucesso.\n\n` +
      `Tempo: ${duration}\n` +
      `${store.selectedDatabase} → ${targetConnection.value.name} / ${targetDatabase.value}`
    )
    emit("copy-done", {
      targetHost: targetConnection.value.host,
      targetPort: Number(targetConnection.value.port) || 3306,
      targetDatabase: targetDatabase.value
    })

  } catch (e) {
    if (cancelled.value || isCancelledError(e))
      return
    console.error(e)
    alert("Erro ao copiar database.\n\nDetalhes: " + e)

  } finally {
    if (typeof stopDump === "function")
      stopDump()
    if (typeof stopCopy === "function")
      stopCopy()
    stopTimer(false)
    copying.value = false
    emit("update:copying", false)
    copyPhase.value = "dump"
    progress.value = 0
    currentTable.value = ""
    exportedCount.value = 0
    totalTables.value = 0
  }
}

async function cancelExport() {
  cancelled.value = true
  try {
    await invoke("cancel_dump")
  } catch (e) {
    console.error(e)
  }
}

async function cancelCopy() {
  cancelled.value = true
  try {
    await invoke("cancel_dump")
  } catch (e) {
    console.error(e)
  }
}

watch(
  () => props.importPending,
  (pending) => {
    if (pending)
      importConfirmNonEmpty.value = false
  }
)

watch(
  () => props.importing,
  (value, prev) => {
    if (value && !prev)
      startTimer()
    else if (!value && prev)
      stopTimer(false)
  }
)

watch(targetIndex, () => {
  loadTargetDatabases()
})

watch(targetDatabase, () => {
  refreshTargetTableCount()
})

watch(destinationMode, (mode) => {
  if (mode === "connection")
    connections.load()
})

onMounted(() => {
  syncDraftFromStore()
  connections.load()
})

onUnmounted(() => {
  stopTimer(false)
})

watch(
  () => store.selectedDatabase,
  () => {
    fileName.value = defaultFileName.value
    syncDraftFromStore()
  }
)

watch(
  () => defaultsStore.rules,
  () => {
    if (!exporting.value && !copying.value)
      syncDraftFromStore()
  },
  { deep: true }
)

</script>

<template>

<aside
  class="export-panel"
  :style="{ '--accent': store.connection?.color || '#3a7afe' }"
>
  <div class="export-panel-header">
    <h3 class="export-panel-title">
      {{ panelTitle }}
    </h3>
    <p class="export-panel-subtitle">
      {{ store.selectedDatabase }}
    </p>
  </div>

  <div
    v-if="importPending && !importing"
    class="export-panel-body"
  >
    <section class="export-section">
      <div class="export-section-title">Arquivo</div>
      <p class="export-import-file" :title="importFileName">
        {{ importFileName || "arquivo.sql" }}
      </p>
    </section>

    <div
      v-if="databaseHasTables"
      class="export-import-warning"
      role="alert"
    >
      <i class="ri-error-warning-line" aria-hidden="true"></i>
      <div>
        <strong>Banco não está vazio</strong>
        <p>
          Existem {{ tables?.length || 0 }} tabela(s) em
          <code>{{ store.selectedDatabase }}</code>.
          A importação pode sobrescrever ou conflitar com dados existentes.
        </p>
      </div>
    </div>

    <div
      v-else
      class="export-import-ok"
    >
      Banco vazio — seguro para importar.
    </div>

    <div class="export-panel-footer">
      <label
        v-if="databaseHasTables"
        class="export-import-confirm"
      >
        <input
          v-model="importConfirmNonEmpty"
          type="checkbox"
        >
        <span>
          Confirmo a importação neste banco que não está vazio
        </span>
      </label>
      <button
        type="button"
        class="btn btn--primary export-submit"
        :disabled="!canStartImport"
        @click="emit('start-import')"
      >
        <i class="ri-download-2-line"></i>
        <span>Iniciar importação</span>
      </button>
      <button
        type="button"
        class="btn btn--ghost export-cancel-btn"
        @click="emit('cancel-import')"
      >
        Cancelar
      </button>
    </div>
  </div>

  <div
    v-else-if="importing || exporting || copying"
    class="export-panel-progress"
  >
    <div class="export-progress-loader" aria-hidden="true">
      <i class="ri-loader-4-line dump-spinner"></i>
    </div>

    <div class="export-progress-copy">
      <span class="export-progress-label">
        <template v-if="importing">Importando</template>
        <template v-else-if="copying && copyPhase === 'import'">
          Importando no destino
        </template>
        <template v-else-if="copying">
          Exportando origem
        </template>
        <template v-else>Exportando</template>
      </span>
      <span class="export-progress-table">
        <template v-if="importing">
          {{ importFileName || "Processando arquivo..." }}
        </template>
        <template v-else>
          {{ currentTable || "Preparando..." }}
        </template>
      </span>
      <span class="export-progress-timer" title="Tempo decorrido">
        <i class="ri-time-line" aria-hidden="true"></i>
        {{ elapsedLabel }}
      </span>
    </div>

    <div
      class="export-progress-bar"
      :class="{
        'export-progress-bar--indeterminate':
          importing || (copying && copyPhase === 'import')
      }"
    >
      <div
        class="export-progress-bar-inner"
        :style="
          importing || (copying && copyPhase === 'import')
            ? undefined
            : { width: progress + '%' }
        "
      />
    </div>

    <div class="export-progress-meta">
      <span v-if="importing">Em andamento</span>
      <span v-else-if="copying && copyPhase === 'import'">Import</span>
      <span v-else>{{ progress }}%</span>

      <span v-if="importing || (copying && copyPhase === 'import')">
        Aguarde…
      </span>
      <span v-else>
        {{ exportedCount }}/{{ totalTables || "…" }} tabelas
      </span>
    </div>

    <button
      v-if="importing"
      type="button"
      class="btn btn--ghost export-cancel-btn"
      @click="emit('cancel-import')"
    >
      Cancelar import
    </button>
    <button
      v-else-if="copying"
      type="button"
      class="btn btn--ghost export-cancel-btn"
      @click="cancelCopy"
    >
      Cancelar cópia
    </button>
    <button
      v-else
      type="button"
      class="btn btn--ghost export-cancel-btn"
      @click="cancelExport"
    >
      Cancelar export
    </button>
  </div>

  <div
    v-else
    class="export-panel-body"
  >
    <section class="export-section">
      <div class="export-section-title">Destino</div>
      <div class="export-mode-toggle">
        <button
          type="button"
          class="export-mode-btn"
          :class="{ 'export-mode-btn--active': destinationMode === 'file' }"
          @click="destinationMode = 'file'"
        >
          Arquivo .sql
        </button>
        <button
          type="button"
          class="export-mode-btn"
          :class="{ 'export-mode-btn--active': destinationMode === 'connection' }"
          @click="destinationMode = 'connection'"
        >
          Outra conexão
        </button>
      </div>
    </section>

    <section
      v-if="destinationMode === 'file'"
      class="export-section"
    >
      <div class="export-section-title">Arquivo</div>

      <label class="export-field">
        <span class="export-field-label">Nome</span>
        <input
          v-model="fileName"
          class="field-input"
          placeholder="database_datetime.sql"
        >
      </label>

      <label class="export-field">
        <span class="export-field-label">Pasta</span>
        <div class="export-path-row">
          <input
            v-model="saveDir"
            class="field-input"
            placeholder="Escolher ao confirmar"
            readonly
          >
          <button
            type="button"
            class="btn btn--ghost btn--icon"
            title="Escolher pasta"
            @click="chooseSaveDir"
          >
            <i class="ri-folder-open-line"></i>
          </button>
        </div>
      </label>

      <p class="export-hint">
        Vazio → <code>{{ defaultFileName }}</code>
      </p>
    </section>

    <section
      v-else
      class="export-section"
    >
      <div class="export-section-title">Conexão destino</div>

      <label class="export-field">
        <span class="export-field-label">Conexão</span>
        <select v-model="targetIndex" class="field-input">
          <option value="" disabled>Selecionar conexão</option>
          <option
            v-for="(c, index) in connections.favorites"
            :key="index"
            :value="String(index)"
          >
            {{ c.name }} ({{ c.host }}:{{ c.port || 3306 }})
          </option>
        </select>
      </label>

      <label class="export-field">
        <span class="export-field-label">Database</span>
        <div class="export-path-row">
          <select
            v-model="targetDatabase"
            class="field-input"
            :disabled="!targetConnection || loadingTargets"
          >
            <option value="" disabled>
              {{ loadingTargets ? "Carregando..." : "Selecionar database" }}
            </option>
            <option
              v-for="db in targetDatabases"
              :key="db.name || db"
              :value="db.name || db"
            >
              {{ db.name || db }}
            </option>
          </select>
          <button
            type="button"
            class="btn btn--ghost btn--icon"
            title="Atualizar databases"
            :disabled="!targetConnection || loadingTargets"
            @click="refreshTargetDatabases"
          >
            <i
              class="ri-refresh-line"
              :class="{ 'dump-spinner': loadingTargets }"
            ></i>
          </button>
        </div>
      </label>

      <p
        v-if="sameTargetAsSource"
        class="export-import-warning"
        role="alert"
      >
        <i class="ri-error-warning-line" aria-hidden="true"></i>
        <span>Origem e destino são o mesmo database.</span>
      </p>

      <div
        v-else-if="targetHasTables"
        class="export-import-warning"
        role="alert"
      >
        <i class="ri-error-warning-line" aria-hidden="true"></i>
        <div>
          <strong>Destino não está vazio</strong>
          <p>
            Existem {{ targetTableCount }} tabela(s) em
            <code>{{ targetDatabase }}</code>.
          </p>
        </div>
      </div>

      <div
        v-else-if="targetDatabase && targetTableCount === 0"
        class="export-import-ok"
      >
        Destino vazio — seguro para copiar.
      </div>
    </section>

    <section class="export-section">
      <div class="export-section-head">
        <div class="export-section-title">Padrões de exclusão</div>
        <button
          type="button"
          class="btn btn--ghost btn--small"
          @click="applyDraftRulesToSelection"
        >
          Aplicar
        </button>
      </div>

      <p class="export-hint">
        Escolha o que desmarcar: S estrutura, C dados, D drop.
      </p>

      <div class="export-rules">
        <div
          v-for="(rule, index) in draftRules"
          :key="index"
          class="export-rule"
        >
          <div class="export-rule-main">
            <span class="export-rule-type">
              {{ rule.type === "prefix" ? "Inicia" : "Termina" }}
            </span>
            <code class="export-rule-value">{{ rule.value }}</code>
            <span class="export-rule-flags" :title="'Exclui: ' + ruleExcludeLabel(rule)">
              {{ ruleExcludeLabel(rule) }}
            </span>
          </div>
          <button
            type="button"
            class="export-rule-remove"
            title="Remover regra"
            @click="removeDraftRule(index)"
          >
            <i class="ri-close-line"></i>
          </button>
        </div>

        <p
          v-if="!draftRules.length"
          class="export-hint"
        >
          Nenhum padrão definido.
        </p>
      </div>

      <div class="export-rule-form">
        <select v-model="newRuleType" class="field-input">
          <option value="suffix">Termina com</option>
          <option value="prefix">Inicia com</option>
        </select>
        <input
          v-model="newRuleValue"
          class="field-input"
          placeholder="log ou bi_"
          @keydown.enter.prevent="addDraftRule"
        >
        <div class="export-rule-checks">
          <label class="export-check" title="Excluir structure">
            <input type="checkbox" v-model="newRuleExcludeStructure">
            S
          </label>
          <label class="export-check" title="Excluir content">
            <input type="checkbox" v-model="newRuleExcludeContent">
            C
          </label>
          <label class="export-check" title="Excluir drop">
            <input type="checkbox" v-model="newRuleExcludeDrop">
            D
          </label>
        </div>
        <button
          type="button"
          class="btn btn--ghost"
          @click="addDraftRule"
        >
          Add
        </button>
      </div>
    </section>

    <div class="export-panel-footer">
      <p class="export-selection-summary">
        <template v-if="hasSelectedTables">
          {{ exportTables.length }} tabela(s) · {{ formatSize(estimatedBytes) }}
        </template>
        <template v-else>
          Nenhuma tabela selecionada
        </template>
      </p>

      <label
        v-if="destinationMode === 'connection' && targetHasTables"
        class="export-import-confirm"
      >
        <input
          v-model="copyConfirmNonEmpty"
          type="checkbox"
        >
        <span>
          Confirmo a cópia para este banco que não está vazio
        </span>
      </label>

      <button
        type="button"
        class="btn btn--primary export-submit"
        :disabled="!canStartAction"
        @click="startAction"
      >
        <i
          :class="destinationMode === 'connection' ? 'ri-file-copy-line' : 'ri-upload-2-line'"
        ></i>
        <span>
          {{ destinationMode === "connection" ? "Iniciar cópia" : "Exportar" }}
        </span>
      </button>
    </div>
  </div>
</aside>

</template>

<style scoped>

.export-panel{
  width: 320px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  min-height: 0;
  border-radius: 16px;
  border: 1px solid color-mix(in srgb, var(--accent, #3a7afe) 35%, #1f2937 65%);
  background:
    radial-gradient(
      circle at top right,
      color-mix(in srgb, var(--accent, #3a7afe) 12%, transparent),
      transparent 55%
    ),
    rgba(2, 6, 23, 0.96);
  overflow: hidden;
}

.export-panel-header{
  padding: 14px 14px 10px;
  border-bottom: 1px solid #1f2937;
}

.export-panel-title{
  margin: 0;
  font-size: 14px;
  font-weight: 600;
}

.export-panel-subtitle{
  margin: 4px 0 0;
  font-size: 12px;
  color: #9ca3af;
  word-break: break-all;
}

.export-panel-body{
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 12px 14px 14px;
  overflow: auto;
}

.export-panel-progress{
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: stretch;
  justify-content: center;
  gap: 14px;
  padding: 24px 18px;
}

.export-progress-loader{
  width: 44px;
  height: 44px;
  border-radius: 999px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  align-self: center;
  background: color-mix(in srgb, var(--accent, #3a7afe) 18%, transparent);
  color: color-mix(in srgb, var(--accent, #3a7afe) 75%, #e5e7eb 25%);
  font-size: 22px;
}

.dump-spinner{
  display: inline-block;
  animation: dump-spin 0.8s linear infinite;
}

@keyframes dump-spin{
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.export-progress-copy{
  display: flex;
  flex-direction: column;
  gap: 4px;
  text-align: center;
}

.export-progress-label{
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.12em;
  color: #9ca3af;
}

.export-progress-table{
  font-size: 14px;
  font-weight: 500;
  color: #e5e7eb;
  word-break: break-all;
}

.export-progress-timer{
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  margin-top: 6px;
  font-variant-numeric: tabular-nums;
  font-size: 18px;
  font-weight: 600;
  color: color-mix(in srgb, var(--accent, #3a7afe) 55%, #e5e7eb 45%);
}

.export-progress-bar{
  height: 6px;
  border-radius: 999px;
  background: #111827;
  overflow: hidden;
}

.export-progress-bar-inner{
  height: 100%;
  border-radius: 999px;
  background: var(--accent, #3a7afe);
  transition: width 0.25s ease-out;
}

.export-progress-bar--indeterminate .export-progress-bar-inner{
  width: 40%;
  animation: export-progress-slide 1.2s ease-in-out infinite;
}

@keyframes export-progress-slide {
  0% { transform: translateX(-120%); }
  100% { transform: translateX(320%); }
}

.export-progress-meta{
  display: flex;
  justify-content: space-between;
  gap: 10px;
  font-size: 12px;
  color: #9ca3af;
}

.export-cancel-btn{
  width: 100%;
  margin-top: 8px;
}

.export-section{
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.export-section-head{
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.export-section-title{
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.12em;
  color: #9ca3af;
  display: flex;
  align-items: center;
  gap: 8px;
}

.export-mode-toggle{
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 6px;
}

.export-mode-btn{
  border-radius: 10px;
  border: 1px solid #1f2937;
  background: rgba(15, 23, 42, 0.9);
  color: #9ca3af;
  padding: 8px 10px;
  font-size: 12px;
  cursor: pointer;
}

.export-mode-btn--active{
  border-color: color-mix(in srgb, var(--accent, #3a7afe) 55%, #1f2937 45%);
  background: color-mix(in srgb, var(--accent, #3a7afe) 16%, rgba(15, 23, 42, 0.95));
  color: #e5e7eb;
}

.export-field{
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.export-field-label{
  font-size: 11px;
  color: #9ca3af;
}

.export-path-row{
  display: flex;
  gap: 6px;
}

.export-path-row .field-input{
  flex: 1;
  min-width: 0;
}

.export-hint{
  margin: 0;
  font-size: 11px;
  color: #6b7280;
  line-height: 1.4;
}

.export-hint code,
.export-rule-value{
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 11px;
  color: #93c5fd;
}

.export-rules{
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-height: 140px;
  overflow: auto;
}

.export-rule{
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 7px 8px;
  border-radius: 10px;
  background: rgba(15, 23, 42, 0.9);
  border: 1px solid #1f2937;
}

.export-rule-main{
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 6px;
}

.export-rule-type{
  font-size: 11px;
  color: #9ca3af;
  flex-shrink: 0;
}

.export-rule-value{
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.export-rule-flags{
  font-size: 10px;
  letter-spacing: 0.08em;
  padding: 2px 6px;
  border-radius: 999px;
  border: 1px solid #374151;
  color: #9ca3af;
  flex-shrink: 0;
}

.export-rule-remove{
  border: none;
  background: transparent;
  color: #9ca3af;
  cursor: pointer;
  padding: 2px;
}

.export-rule-form{
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 6px;
}

.export-rule-form > .field-input:first-child {
  grid-column: 1 / 2;
}

.export-rule-form > .field-input:nth-child(2) {
  grid-column: 2 / 3;
}

.export-rule-checks{
  display: flex;
  align-items: center;
  gap: 8px;
  grid-column: 1 / 2;
}

.export-rule-form > .btn {
  grid-column: 2 / 3;
}

.export-check{
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
  color: #9ca3af;
  cursor: pointer;
}

.export-panel-footer{
  margin-top: auto;
  padding-top: 4px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.export-selection-summary{
  margin: 0;
  font-size: 12px;
  color: #9ca3af;
}

.export-import-file{
  margin: 0;
  padding: 10px 12px;
  border-radius: 10px;
  border: 1px solid #1f2937;
  background: rgba(15, 23, 42, 0.9);
  font-size: 13px;
  font-weight: 500;
  color: #e5e7eb;
  word-break: break-all;
}

.export-import-warning{
  display: flex;
  gap: 10px;
  align-items: flex-start;
  padding: 12px;
  border-radius: 12px;
  border: 1px solid color-mix(in srgb, #f59e0b 45%, #1f2937 55%);
  background: color-mix(in srgb, #f59e0b 12%, rgba(15, 23, 42, 0.95));
  color: #fde68a;
  font-size: 12px;
  line-height: 1.45;
}

.export-import-warning i{
  flex-shrink: 0;
  margin-top: 1px;
  font-size: 18px;
  color: #fbbf24;
}

.export-import-warning strong{
  display: block;
  font-size: 13px;
  color: #fef3c7;
}

.export-import-warning p{
  margin: 4px 0 0;
  font-size: 12px;
  line-height: 1.45;
  color: #fde68a;
}

.export-import-warning code{
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 11px;
  color: #fef3c7;
}

.export-import-ok{
  padding: 10px 12px;
  border-radius: 10px;
  border: 1px solid #1f2937;
  background: rgba(15, 23, 42, 0.9);
  font-size: 12px;
  color: #9ca3af;
}

.export-import-confirm{
  display: flex;
  align-items: flex-start;
  gap: 8px;
  font-size: 12px;
  line-height: 1.4;
  color: #e5e7eb;
  cursor: pointer;
}

.export-import-confirm input{
  margin-top: 2px;
  flex-shrink: 0;
}

.export-submit{
  width: 100%;
}

.btn--small{
  font-size: 11px;
  padding-inline: 10px;
}

button[disabled]{
  opacity: 0.55;
  cursor: not-allowed;
}

</style>
