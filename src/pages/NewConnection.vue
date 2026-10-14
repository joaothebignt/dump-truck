<script setup>

import { computed, onMounted, reactive, ref } from "vue"
import { useRoute, useRouter } from "vue-router"
import { invoke } from "@tauri-apps/api/core"

import { useConnectionsStore } from "../stores/connections"

const router = useRouter()
const route = useRoute()
const store = useConnectionsStore()

const editingIndex = ref(null)
const testing = ref(false)
const testMessage = ref(null)
const original = ref(null)

const form = reactive({

  name: "",
  host: "127.0.0.1", // IP, hostname ou domínio externo
  port: 3306,
  username: "",
  password: "",
  color: "#3a7afe"

})

const showPassword = ref(false)

const isEditing = computed(() => editingIndex.value !== null)

const hasChanges = computed(() => {
  if (!isEditing.value || !original.value)
    return false

  const current = normalizeConnection(form)
  const base = original.value

  return (
    current.name !== base.name ||
    current.host !== base.host ||
    current.port !== base.port ||
    current.username !== base.username ||
    current.password !== base.password ||
    current.color !== base.color
  )
})

function normalizeConnection(connection) {
  return {
    name: String(connection.name || "").trim(),
    host: String(connection.host || "").trim(),
    port: Number(connection.port) || 3306,
    username: String(connection.username || ""),
    password: String(connection.password || ""),
    color: connection.color || "#3a7afe"
  }
}

onMounted(() => {

  if (route.params.index !== undefined) {

    const idx = Number(route.params.index)
    const existing = store.favorites[idx]

    if (existing) {

      editingIndex.value = idx

      const normalized = normalizeConnection(existing)
      original.value = { ...normalized }

      form.name = normalized.name
      form.host = normalized.host
      form.port = normalized.port
      form.username = normalized.username
      form.password = normalized.password
      form.color = normalized.color

    }
  }

})

async function testConnection() {
  if (!form.host?.trim()) {
    testMessage.value = { ok: false, text: "Informe o host." }
    return
  }
  testing.value = true
  testMessage.value = null
  try {
    await invoke("connect_db", {
      host: form.host.trim(),
      port: Number(form.port) || 3306,
      user: form.username || "",
      pass: form.password || ""
    })
    testMessage.value = { ok: true, text: "Conexão OK." }
  } catch (e) {
    testMessage.value = { ok: false, text: String(e) }
  } finally {
    testing.value = false
  }
}

function saveChanges() {
  if (!isEditing.value || !hasChanges.value)
    return

  store.update(editingIndex.value, normalizeConnection(form))
  router.push("/")
}

function saveAndFavorite() {
  store.add(normalizeConnection(form))
  router.push("/")
}

</script>

<template>

  <div class="new-connection">
    <div class="new-connection-header">
      <div>
        <h2 class="title">
          {{ editingIndex !== null ? "Editar conexão" : "Nova conexão" }}
        </h2>
        <p class="subtitle">
          Defina os detalhes da conexão e teste antes de salvar.
        </p>
      </div>
    </div>

    <div class="card">
      <div class="form-grid">
        <label class="field">
          <span class="field-label">Nome da conexão</span>
          <input
            v-model="form.name"
            class="field-input"
            placeholder="Produção, staging, local..."
          />
        </label>

        <label class="field field--host-port">
          <span class="field-label">Host e porta</span>
          <div class="host-port-wrapper">
            <input
              v-model="form.host"
              class="field-input host-input"
              placeholder="Ex.: 192.168.0.10 ou db.exemplo.com"
            />
            <span class="host-port-separator">:</span>
            <input
              v-model="form.port"
              class="field-input port-input"
              placeholder="3306"
            />
          </div>
        </label>

        <label class="field">
          <span class="field-label">Usuário</span>
          <input
            v-model="form.username"
            class="field-input"
            placeholder="root, app_user..."
          />
        </label>

        <label class="field">
          <span class="field-label">Senha</span>
          <div class="password-wrapper">
            <input
              :type="showPassword ? 'text' : 'password'"
              v-model="form.password"
              class="field-input"
              placeholder="Digite a senha"
            />
            <button
              type="button"
              class="password-toggle"
              @click="showPassword = !showPassword"
            >
              {{ showPassword ? "Ocultar" : "Mostrar" }}
            </button>
          </div>
        </label>

        <label class="field field--small">
          <span class="field-label">Cor</span>
          <input
            v-model="form.color"
            type="color"
            class="color-input"
          >
        </label>
      </div>

      <div
        v-if="testMessage"
        :class="['test-message', testMessage.ok ? 'success' : 'error']"
      >
        {{ testMessage.text }}
      </div>

      <div class="actions">
        <button
          class="btn btn--ghost"
          @click="testConnection"
          :disabled="testing"
        >
          {{ testing ? "Testando…" : "Testar conexão" }}
        </button>
        <div class="actions-right">
          <template v-if="isEditing">
            <button
              class="btn btn--primary"
              @click="saveChanges"
              :disabled="!hasChanges"
            >
              Salvar alterações
            </button>
            <button
              class="btn btn--ghost"
              @click="router.push('/')"
            >
              Cancelar
            </button>
          </template>
          <template v-else>
            <button
              class="btn btn--primary"
              @click="saveAndFavorite"
            >
              Salvar e favoritar
            </button>
            <button
              class="btn btn--ghost"
              @click="router.push('/')"
            >
              Cancelar
            </button>
          </template>
        </div>
      </div>
    </div>
  </div>

</template>

<style scoped>
.new-connection {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding-bottom: 20px;
}

.new-connection-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}

.title {
  margin: 0 0 4px;
  font-size: 20px;
  font-weight: 500;
}

.subtitle {
  margin: 0;
  font-size: 13px;
  color: #9ca3af;
}

.card {
  border-radius: 18px;
  padding: 20px 18px 18px;
  background: radial-gradient(circle at top left, #020617, #020617);
  border: 1px solid #111827;
  box-shadow: 0 22px 45px rgba(0, 0, 0, 0.75);
}

.form-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 14px 16px;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.field-label {
  font-size: 12px;
  color: #9ca3af;
}

.field--host-port {
  grid-column: span 2;
}

.host-port-wrapper {
  display: flex;
  align-items: stretch;
  border-radius: 999px;
  border: 1px solid #27272a;
  background: #020617;
  overflow: hidden;
}

.host-input,
.port-input {
  border: none;
  background: transparent;
  padding: 8px 12px;
}

.host-input {
  flex: 1;
}

.port-input {
  width: 96px;
  text-align: center;
}

.host-port-separator {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 0 4px;
  font-size: 13px;
  color: #6b7280;
}

.password-wrapper {
  position: relative;
  display: flex;
  align-items: center;
}

.password-toggle {
  position: absolute;
  right: 10px;
  background: transparent;
  border: none;
  color: #9ca3af;
  font-size: 11px;
  cursor: pointer;
  padding: 0;
}

.field--small {
  max-width: 180px;
}

.color-input {
  border-radius: 999px;
  padding: 0;
  border: 1px solid #27272a;
  background: #000;
  width: 100%;
  height: 36px;
  cursor: pointer;
}

.test-message {
  margin-top: 12px;
  padding: 8px 10px;
  border-radius: 8px;
  font-size: 13px;
}

.test-message {
  padding: 8px 10px;
  border-radius: 4px;
  font-size: 14px;
  margin-bottom: 8px;
}
.test-message.success {
  background: rgba(56, 189, 124, 0.2);
  color: #38bd7c;
}
.test-message.error {
  background: rgba(248, 113, 113, 0.2);
  color: #f87171;
}

.actions {
  margin-top: 18px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.actions-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

@media (max-width: 900px) {
  .form-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

@media (max-width: 640px) {
  .form-grid {
    grid-template-columns: minmax(0, 1fr);
  }
  .field--small {
    max-width: none;
  }
}
</style>