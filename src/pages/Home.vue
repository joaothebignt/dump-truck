<script setup>

import { onMounted, ref } from "vue"
import { useRouter } from "vue-router"
import { invoke } from "@tauri-apps/api/core"

import { useConnectionsStore } from "../stores/connections"
import { useDatabaseStore } from "../stores/database"

const router = useRouter()
const store = useConnectionsStore()
const dbStore = useDatabaseStore()
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

function newConnection() {

  router.push("/connection/new")

}

async function connect(connection) {

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

  }

}

</script>

<template>

<h1>DB Dumper</h1>

<button @click="newConnection">
Nova conexão
</button>

<h2>Favoritas</h2>

<div
  v-for="(c, index) in store.favorites"
  :key="connectionKey(c) + '-' + index"
  class="connection-item"
  :class="{ offline: isOffline(c) }"
>
  <span class="connection-info">
    {{ c.name }} – {{ c.host }}
    <span v-if="isOffline(c)" class="connection-offline">Offline</span>
  </span>
  <button @click="connect(c)" :disabled="isOffline(c)">
    Conectar
  </button>
</div>

</template>

<style scoped>
.connection-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 0;
  border-bottom: 1px solid #333;
}
.connection-item.offline {
  opacity: 0.7;
}
.connection-info {
  display: flex;
  align-items: center;
  gap: 6px;
}
.connection-offline {
  font-size: 10px;
  text-transform: uppercase;
  color: #888;
  font-weight: 500;
}
.connection-item button:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
</style>