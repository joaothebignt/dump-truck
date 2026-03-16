<script setup>

import { computed, onMounted, ref, watch } from "vue"
import { useRouter } from "vue-router"
import { invoke } from "@tauri-apps/api/core"

import { useConnectionsStore } from "../stores/connections"
import { useDatabaseStore } from "../stores/database"

const store = useConnectionsStore()
const dbStore = useDatabaseStore()
const router = useRouter()

const hasConnection = computed(() => !!dbStore.connection)
const connectionStatus = ref({})

function connectionKey(c) {
  return `${c.host || ""}:${c.port ?? 3306}`
}

function isOffline(c) {
  const key = connectionKey(c)
  return connectionStatus.value[key] === false
}

async function refreshConnectionStatus() {
  const next = { ...connectionStatus.value }
  const list = store.favorites || []
  const results = await Promise.allSettled(
    list.map(async (c) => {
      const key = connectionKey(c)
      try {
        const ok = await invoke("check_connection", {
          host: String(c.host || ""),
          port: Number(c.port) || 3306
        })
        return { key, ok }
      } catch {
        return { key, ok: false }
      }
    })
  )
  results.forEach((r) => {
    if (r.status === "fulfilled" && r.value) {
      next[r.value.key] = r.value.ok
    }
  })
  connectionStatus.value = next
}

onMounted(() => {
  store.load()
  refreshConnectionStatus()
})

watch(() => store.favorites, () => refreshConnectionStatus(), { deep: true })

function newConnection(){
  router.push("/connection/new")
}

async function connect(connection){

  if (!connection || !connection.host) {

    alert("Conexão inválida: host não definido. Edite ou crie uma nova conexão.")
    return

  }

  try{

    const conn = await invoke("connect_db",{
      host: connection.host,
      port: Number(connection.port),
      user: connection.username,
      pass: connection.password
    })

    dbStore.setConnection(conn)
    dbStore.selectedDatabase = null
    dbStore.tables = []

    router.push("/database")

  }catch(e){

    console.error(e)
    alert("Erro ao conectar ao banco de dados.\n\nDetalhes: " + e)

  }

}

function disconnect(){

  dbStore.clearConnection()
  router.push("/")

}

function editConnection(index){

  router.push(`/connection/edit/${index}`)

}

</script>

<template>

<div class="sidebar-container">

  <div class="section">

    <div class="section-title">
      Connections
    </div>

    <div
      class="connection"
      v-for="(c, index) in store.favorites"
      :key="connectionKey(c) + '-' + index"
      :class="{ offline: isOffline(c) }"
      :style="{ borderLeftColor: c.color || '#3a7afe' }"
    >

      <span
        class="connection-name"
        @click="connect(c)"
      >
        {{ c.name }}
        <span v-if="isOffline(c)" class="connection-offline">Offline</span>
      </span>

      <button
        class="edit-btn"
        @click.stop="editConnection(index)"
        title="Editar conexão"
      >
        ✎
      </button>

      <button
        class="delete-btn"
        @click.stop="store.remove(index)"
        title="Excluir conexão"
      >
        ×
      </button>

    </div>

    <button
      class="new-btn"
      @click="newConnection"
    >
      + Nova conexão
    </button>

    <button
      v-if="hasConnection"
      class="disconnect-btn"
      @click="disconnect"
    >
      Fechar conexão atual
    </button>

  </div>

</div>

</template>

<style scoped>

.sidebar-container{
  padding:10px;
}

.section-title{
  font-size:12px;
  text-transform:uppercase;
  margin-bottom:8px;
  color:#aaa;
}

.connection{
  padding:6px 8px;
  border-radius:4px;
  cursor:pointer;
  display:flex;
  align-items:center;
  justify-content:space-between;
  border-left:3px solid transparent;
}

.connection:hover{
  background:#333;
}

.connection.offline .connection-name {
  opacity: 0.85;
}

.connection-offline {
  display: inline-block;
  margin-left: 6px;
  font-size: 10px;
  text-transform: uppercase;
  color: #888;
  font-weight: 500;
}

.delete-btn{
  background:transparent;
  border:none;
  color:#888;
  cursor:pointer;
  font-size:14px;
  padding:0 4px;
}

.delete-btn:hover{
  color:#f88;
}

.edit-btn{
  background:transparent;
  border:none;
  color:#888;
  cursor:pointer;
  font-size:13px;
  padding:0 4px;
}

.edit-btn:hover{
  color:#3a7afe;
}

.new-btn{
  margin-top:10px;
  width:100%;
  background:#3a7afe;
  border:none;
  color:white;
  padding:6px;
  border-radius:4px;
  cursor:pointer;
}

.disconnect-btn{
  margin-top:6px;
  width:100%;
  background:#555;
  border:none;
  color:white;
  padding:6px;
  border-radius:4px;
  cursor:pointer;
}

</style>