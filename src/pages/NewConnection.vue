<script setup>

import { onMounted, reactive, ref } from "vue"
import { useRoute, useRouter } from "vue-router"
import { invoke } from "@tauri-apps/api/core"

import { useConnectionsStore } from "../stores/connections"

const router = useRouter()
const route = useRoute()
const store = useConnectionsStore()

const editingIndex = ref(null)
const testing = ref(false)
const testMessage = ref(null)

const form = reactive({

  name: "",
  host: "127.0.0.1", // pode ser IP ou hostname na rede local
  port: 3306,
  username: "",
  password: "",
  color: "#3a7afe",
  saveFavorite: false

})

onMounted(() => {

  if (route.params.index !== undefined) {

    const idx = Number(route.params.index)
    const existing = store.favorites[idx]

    if (existing) {

      editingIndex.value = idx

      form.name = existing.name
      form.host = existing.host
      form.port = existing.port
      form.username = existing.username
      form.password = existing.password
      form.color = existing.color || "#3a7afe"
      form.saveFavorite = true

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

function save() {

  if (editingIndex.value !== null) {

    store.update(editingIndex.value, { ...form })

  } else if (form.saveFavorite) {

    store.add({ ...form })

  }

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

        <label class="field">
          <span class="field-label">Host</span>
          <input
            v-model="form.host"
            class="field-input"
            placeholder="Ex.: 192.168.0.10"
          />
        </label>

        <label class="field field--small">
          <span class="field-label">Porta</span>
          <input
            v-model="form.port"
            class="field-input"
            placeholder="3306"
          />
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
          <input
            type="password"
            v-model="form.password"
            class="field-input"
            placeholder="Digite a senha"
          />
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

      <label class="favorite-toggle">
        <input
          v-model="form.saveFavorite"
          type="checkbox"
        >
        <span>Salvar como favorita</span>
      </label>

      <div
        v-if="testMessage"
        :class="['test-message', testMessage.ok ? 'success' : 'error']"
      >
        {{ testMessage.text }}
      </div>

      <div class="actions">
        <button
          class="btn btn--ghost"
          @click="router.push('/')"
        >
          Cancelar
        </button>
        <div class="actions-right">
          <button
            class="btn btn--ghost"
            @click="testConnection"
            :disabled="testing"
          >
            {{ testing ? "Testando…" : "Testar conexão" }}
          </button>
          <button
            class="btn btn--primary"
            @click="save"
          >
            Salvar
          </button>
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

.favorite-toggle {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  margin-top: 16px;
  font-size: 13px;
  color: #e5e7eb;
}

.favorite-toggle input {
  margin: 0;
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