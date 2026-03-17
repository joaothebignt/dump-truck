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

function editConnection(index) {
  router.push(`/connection/edit/${index}`)
}

function removeConnection(index) {
  if (!confirm("Remover esta conexão favorita?")) {
    return
  }
  store.remove(index)
  refreshConnectionStatus()
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

    dbStore.setConnection({
      url: conn,
      name: connection.name,
      host: connection.host,
      color: connection.color,
      port: connection.port,
      username: connection.username
    })
    dbStore.selectedDatabase = null
    dbStore.tables = []

    router.push("/database")

  }catch(e){

    console.error(e)

  }

}

</script>

<template>

  <div class="home">
    <div class="home-header">
      <div class="home-title-group">
        <h1 class="home-title">Suas conexões</h1>
        <p class="home-subtitle">
          Escolha uma conexão favorita para começar.
        </p>
      </div>
      <button class="btn btn--primary" @click="newConnection">
        Nova conexão
      </button>
    </div>

    <div
      v-if="!store.favorites || !store.favorites.length"
      class="home-empty"
    >
      <p class="home-empty-title">Nenhuma conexão favorita ainda</p>
      <p class="home-empty-text">
        Crie uma nova conexão para salvar e acessar rapidamente seus bancos.
      </p>
      <button class="btn btn--primary" @click="newConnection">
        Criar primeira conexão
      </button>
    </div>

    <div
      v-else
      class="connections-grid"
    >
      <div
        v-for="(c, index) in store.favorites"
        :key="connectionKey(c) + '-' + index"
        class="connection-card"
        :class="{
          'connection-card--offline': isOffline(c)
        }"
        :style="{
          '--accent': isOffline(c) ? 'transparent' : (c.color || '#3a7afe')
        }"
        @click="!isOffline(c) && connect(c)"
      >
        <div class="connection-card-badge">
          <span
            class="status-badge"
            :class="isOffline(c) ? 'status-badge--offline' : 'status-badge--online'"
          >
            {{ isOffline(c) ? 'Offline' : 'Online' }}
          </span>
          <div class="connection-actions">
            <button
              class="btn btn--ghost btn--icon"
              @click.stop="editConnection(index)"
              title="Editar conexão"
            >
              <i class="ri-edit-2-line icon" aria-hidden="true"></i>
            </button>
            <button
              class="btn btn--ghost btn--icon connection-remove"
              @click.stop="removeConnection(index)"
              title="Remover conexão"
            >
              <i class="ri-delete-bin-6-line icon" aria-hidden="true"></i>
            </button>
          </div>
        </div>

        <div class="connection-card-body">
          <div class="connection-meta-bottom">
            <div class="connection-meta-text">
              <span class="connection-name">
                {{ c.name || 'Sem nome' }}
              </span>
              <span class="connection-ip">
                {{ c.host || 'Host indefinido' }}{{ c.port ? ':' + c.port : '' }}
              </span>
            </div>
            <button
              class="btn btn--ghost btn--icon connection-connect"
              :disabled="isOffline(c)"
              @click.stop="connect(c)"
            >
              <i class="ri-plug-line" aria-hidden="true"></i>
            </button>
          </div>
        </div>
      </div>
    </div>
  </div>

</template>

<style scoped>
.home {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.home-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 20px;
}

.home-title-group {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.home-title {
  margin: 0;
  font-size: 22px;
  font-weight: 500;
  letter-spacing: 0.02em;
}

.home-subtitle {
  margin: 0;
  font-size: 13px;
  color: #9ca3af;
}

.home-empty {
  margin-top: 40px;
  border-radius: 20px;
  border: 1px dashed #27272a;
  padding: 32px 24px;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 12px;
  max-width: 420px;
  background: radial-gradient(circle at top left, #020617, #020617);
}

.home-empty-title {
  margin: 0;
  font-size: 16px;
  font-weight: 500;
}

.home-empty-text {
  margin: 0;
  font-size: 13px;
  color: #9ca3af;
}

.connections-grid {
  flex: 1;
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  grid-auto-rows: minmax(0, 1fr);
  gap: 16px;
  align-content: flex-start;
}

.connection-card {
  position: relative;
  border-radius: 18px;
  padding: 14px 14px 12px;
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
  min-height: 140px;
  opacity: 1;
  transition: transform 0.12s ease, box-shadow 0.12s ease, opacity 0.12s ease,
    border-color 0.12s ease, background 0.12s ease;
  border: 1px solid color-mix(in srgb, var(--accent, #3a7afe) 70%, #020617 30%);
}

.connection-card::before {
  content: "";
  position: absolute;
  inset: 0;
  border-radius: inherit;
  background: radial-gradient(circle at top left, color-mix(in srgb, var(--accent, #3a7afe) 35%, transparent), transparent);
  opacity: 0.5;
  pointer-events: none;
}

.connection-card:hover {
  transform: translateY(-2px);
  box-shadow:
    0 24px 60px rgba(0, 0, 0, 0.85),
    0 0 0 1px rgba(37, 99, 235, 0.8);
}

.connection-card--offline {
  opacity: 0.5;
  cursor: not-allowed;
  border-color: rgba(63, 63, 70, 0.9);
  box-shadow:
    inset 0 0 0 1px rgba(39, 39, 42, 0.8),
    0 18px 40px rgba(0, 0, 0, 0.7);
}

.connection-card--offline::before {
  opacity: 0;
}

.connection-card-badge {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.status-badge {
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.14em;
  padding: 3px 8px;
  border-radius: 999px;
  border: 1px solid #27272a;
}

.status-badge--online {
  border-color: #22c55e;
  color: #fff;
  background: rgba(34, 197, 94, 0.75);
}

.status-badge--offline {
  border-color: #3f3f46;
  color: #a1a1aa;
  background: rgba(24, 24, 27, 0.9);
}

.connection-card-body {
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  flex: 1;
  padding-top: 14px;
}

.connection-name {
  display: block;
  font-size: 24px;
  font-weight: 600;
  color: #f9fafb;
}

.connection-actions {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  opacity: 0;
  pointer-events: none;
  transition: opacity 0.12s ease;
}

.connection-card:hover .connection-actions {
  opacity: 1;
  pointer-events: auto;
}

.connection-actions .btn--icon {
  padding-inline: 10px;
  padding-block: 6px;
  font-size: 13px;
}

.connection-remove {
  color: #f97373;
}

.icon {
  width: 14px;
  height: 14px;
  display: block;
}

.connection-meta-bottom {
  margin-top: auto;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.connection-meta-text {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.connection-ip {
  font-size: 12px;
  color: #9ca3af;
}

.connection-connect {
  opacity: 0;
  pointer-events: none;
  transition: opacity 0.12s ease, transform 0.12s ease;
}

.connection-card:hover .connection-connect {
  opacity: 1;
  pointer-events: auto;
  transform: translateX(0);
}

.btn--ghost {
  font-size: 12px;
  padding-block: 6px;
}

@media (max-width: 1024px) {
  .connections-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

@media (max-width: 720px) {
  .connections-grid {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>